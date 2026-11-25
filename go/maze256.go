package mazesort

import (
	"math/bits"
	"slices"
)

const (
	smallCutoff    = 256
	insertionLimit = 64
	smallStackSize = 1024
	stackBufSize   = 4096
	blockSize      = 8 // 8 x int64 = 64-byte cache line
)

// Maze256 sorts data in ascending order in-place using the In-Place Maze256
// algorithm: a range-adaptive 8-bit MSD radix sort with tiered stack frames,
// cache-line block-buffered scatter, cycle-leader fallback for skewed data,
// and TZCNT checkpoint bitmasks.
//
// It runs in O(n) time (at most 8 radix levels for 64-bit integers) and uses
// O(1) heap space (0 allocs/op).
func Maze256[T Integer](data []T) {
	n := len(data)
	if n <= smallCutoff {
		slices.Sort(data)
		return
	}
	if handleMonotonic(data) {
		return
	}
	if n <= smallStackSize {
		inPlaceMazeSmall(data)
		return
	}
	if n <= stackBufSize {
		inPlaceMazeMed(data)
		return
	}
	inPlaceMazeLarge(data)
}

//go:noinline
func inPlaceMazeSmall[T Integer](data []T) {
	var stackAux [smallStackSize]T
	stackMazePP(data, stackAux[:len(data)], false)
}

//go:noinline
func inPlaceMazeMed[T Integer](data []T) {
	var stackAux [stackBufSize]T
	stackMazePP(data, stackAux[:len(data)], false)
}

//go:noinline
func inPlaceMazeLarge[T Integer](data []T) {
	var stackAux [stackBufSize]T
	var buf [256][blockSize]T
	var pending [2048]T // Provably bounded: pendingLen <= 256*(blockSize-1)+1 = 1793 < 2048
	inPlaceMaze(data, stackAux[:], &buf, &pending)
}

func inPlaceMaze[T Integer](src, stackAux []T, buf *[256][blockSize]T, pending *[2048]T) {
	n := len(src)
	if n <= insertionLimit {
		insertionSort(src)
		return
	}
	if n <= stackBufSize {
		stackMazePP(src, stackAux[:n], false)
		return
	}

	minVal, maxVal := src[0], src[0]
	for _, v := range src[1:] {
		if v < minVal {
			minVal = v
		} else if v > maxVal {
			maxVal = v
		}
	}
	if minVal == maxVal {
		return
	}

	// Two's-complement uint64 subtraction works uniformly for signed and unsigned T.
	minU := uint64(minVal)
	span := uint64(maxVal) - minU
	bitLen := 64 - bits.LeadingZeros64(span)

	var counts [256]int

	// Direct in-place counting sort when active span fits in 8 bits.
	if bitLen <= 8 {
		for _, v := range src {
			counts[uint8(uint64(v)-minU)]++
		}
		out := 0
		limit := int(span)
		for r := 0; r <= limit; r++ {
			c := counts[r]
			if c > 0 {
				val := T(minU + uint64(r))
				for k := 0; k < c; k++ {
					src[out] = val
					out++
				}
			}
		}
		return
	}

	// Pass 1: Clean single-table histogram (uint8 index enables BCE).
	shift := uint(bitLen - 8)
	for _, v := range src {
		counts[uint8((uint64(v)-minU)>>shift)]++
	}

	// Build prefix sums and 4-register checkpoint bitmask in 256 iterations.
	var offsets [256]int
	var writePos [256]int
	var ends [256]int
	var roomMask [4]uint64
	total := 0
	activeRooms := 0
	lastRoom := uint8(0)
	maxCount := 0

	for r := 0; r < 256; r++ {
		c := counts[r]
		if c > 0 {
			roomMask[r>>6] |= uint64(1) << (uint(r) & 63)
			offsets[r] = total
			writePos[r] = total
			total += c
			ends[r] = total
			lastRoom = uint8(r)
			activeRooms++
			if c > maxCount {
				maxCount = c
			}
		}
	}

	// Pass 2: Adaptive in-place routing.
	if activeRooms > 1 {
		if activeRooms >= 32 && maxCount < (n*3)/4 {
			// Spread-out data: cache-line block-buffered in-place scatter.
			readPos := offsets
			var bufLen [256]uint8
			pendingLen := 0

			for z := 0; z < 4; z++ {
				for rm := roomMask[z]; rm != 0; rm &= rm - 1 {
					r := uint8((z << 6) | bits.TrailingZeros64(rm))
					end := ends[r]

					for readPos[r] < end {
						var v T
						if pendingLen > 0 {
							pendingLen--
							v = pending[pendingLen]
						} else {
							v = src[readPos[r]]
							readPos[r]++
						}

						dest := uint8((uint64(v) - minU) >> shift)
						bl := bufLen[dest]
						buf[dest][bl] = v
						bl++

						if bl == blockSize {
							wp := writePos[dest]
							newWP := wp + blockSize
							rp := readPos[dest]
							if newWP > rp {
								for k := rp; k < newWP; k++ {
									pending[pendingLen] = src[k]
									pendingLen++
								}
								readPos[dest] = newWP
							}
							copy(src[wp:newWP], buf[dest][:])
							writePos[dest] = newWP
							bufLen[dest] = 0
						} else {
							bufLen[dest] = bl
						}
					}
				}
			}

			// Drain remaining pending elements.
			for pendingLen > 0 {
				pendingLen--
				v := pending[pendingLen]
				dest := uint8((uint64(v) - minU) >> shift)
				bl := bufLen[dest]
				buf[dest][bl] = v
				bl++
				if bl == blockSize {
					wp := writePos[dest]
					copy(src[wp:wp+blockSize], buf[dest][:])
					writePos[dest] = wp + blockSize
					bufLen[dest] = 0
				} else {
					bufLen[dest] = bl
				}
			}

			// Flush partial tail blocks.
			for z := 0; z < 4; z++ {
				for rm := roomMask[z]; rm != 0; rm &= rm - 1 {
					r := uint8((z << 6) | bits.TrailingZeros64(rm))
					bl := int(bufLen[r])
					if bl > 0 {
						wp := writePos[r]
						copy(src[wp:wp+bl], buf[r][:bl])
					}
				}
			}
		} else {
			// Skewed or sparse data: zero-write cycle-leader routing.
			for z := 0; z < 4; z++ {
				for rm := roomMask[z]; rm != 0; rm &= rm - 1 {
					r := uint8((z << 6) | bits.TrailingZeros64(rm))
					if r == lastRoom {
						break
					}
					end := ends[r]
					wp := writePos[r]
					for wp < end {
						v := src[wp]
						dest := uint8((uint64(v) - minU) >> shift)
						if dest == r {
							wp++
							continue
						}
						for {
							nextPos := writePos[dest]
							writePos[dest] = nextPos + 1
							src[nextPos], v = v, src[nextPos]
							dest = uint8((uint64(v) - minU) >> shift)
							if dest == r {
								src[wp] = v
								wp++
								break
							}
						}
					}
					writePos[r] = wp
				}
			}
		}
	}

	// Recurse into occupied rooms via TZCNT checkpoint bitmasks.
	for z := 0; z < 4; z++ {
		for rm := roomMask[z]; rm != 0; rm &= rm - 1 {
			r := uint8((z << 6) | bits.TrailingZeros64(rm))
			c := counts[r]
			if c > 1 {
				st := offsets[r]
				inPlaceMaze(src[st:st+c], stackAux, buf, pending)
			}
		}
	}
}

func stackMazePP[T Integer](src, dst []T, inAux bool) {
	n := len(src)
	if n <= insertionLimit {
		insertionSort(src)
		if inAux {
			copy(dst, src)
		}
		return
	}

	minVal, maxVal := src[0], src[0]
	for _, v := range src[1:] {
		if v < minVal {
			minVal = v
		} else if v > maxVal {
			maxVal = v
		}
	}
	if minVal == maxVal {
		if inAux {
			copy(dst, src)
		}
		return
	}

	minU := uint64(minVal)
	span := uint64(maxVal) - minU
	bitLen := 64 - bits.LeadingZeros64(span)

	var counts [256]int

	if bitLen <= 8 {
		for _, v := range src {
			counts[uint8(uint64(v)-minU)]++
		}
		target := src
		if inAux {
			target = dst
		}
		out := 0
		limit := int(span)
		for r := 0; r <= limit; r++ {
			c := counts[r]
			if c > 0 {
				val := T(minU + uint64(r))
				for k := 0; k < c; k++ {
					target[out] = val
					out++
				}
			}
		}
		return
	}

	shift := uint(bitLen - 8)
	for _, v := range src {
		counts[uint8((uint64(v)-minU)>>shift)]++
	}

	var offsets [256]int
	var writePos [256]int
	var roomMask [4]uint64
	total := 0
	for r := 0; r < 256; r++ {
		c := counts[r]
		if c > 0 {
			roomMask[r>>6] |= uint64(1) << (uint(r) & 63)
			offsets[r] = total
			writePos[r] = total
			total += c
		}
	}

	for _, v := range src {
		r := uint8((uint64(v) - minU) >> shift)
		dst[writePos[r]] = v
		writePos[r]++
	}

	for z := 0; z < 4; z++ {
		for rm := roomMask[z]; rm != 0; rm &= rm - 1 {
			r := uint8((z << 6) | bits.TrailingZeros64(rm))
			c := counts[r]
			st := offsets[r]
			if c == 1 {
				if !inAux {
					src[st] = dst[st]
				}
				continue
			}
			stackMazePP(dst[st:st+c], src[st:st+c], !inAux)
		}
	}
}
