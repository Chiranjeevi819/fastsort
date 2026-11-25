package mazesort

import (
	"math/bits"
	"runtime"
	"slices"
	"sync"
	"sync/atomic"
)

const (
	parallelCutoff    = 32768
	parallelMaxProcs  = 32
	parallelLeafLimit = 16384
)

// Maze256Parallel sorts data in ascending order in-place across up to
// runtime.GOMAXPROCS(0) goroutines using a hybrid parallel In-Place Maze256
// algorithm with zero O(n) auxiliary slice allocations.
//
// For inputs smaller than 32,768 elements (or when GOMAXPROCS == 1), it
// delegates directly to [Maze256] with 0 B/op and 0 allocs/op.
//
// For larger inputs, it runs a parallel min/max and monotonicity scan, a
// parallel 8-bit counting sort when span < 256, and a parallel 256-bucket
// histogram that adaptively routes:
//   - Spread-out distributions: cache-line block-buffered 256-way scatter
//     followed by dynamic work-stealing parallel room sorting with per-worker
//     hoisted stack buffers.
//   - Skewed, heavy-tailed, or staircase distributions: median-of-65 branchless
//     BlockQuicksort parallel partitioning into balanced chunks sorted in
//     parallel by [Maze256].
func Maze256Parallel[T Integer](data []T) {
	n := len(data)
	procs := runtime.GOMAXPROCS(0)
	if procs <= 1 || n < parallelCutoff {
		Maze256(data)
		return
	}
	if procs > parallelMaxProcs {
		procs = parallelMaxProcs
	}

	// Phase 1: Parallel min/max + monotonicity scan across procs chunks.
	type chunkStat struct {
		minV, maxV T
		asc, desc  bool
	}
	var stats [parallelMaxProcs]chunkStat
	var wg sync.WaitGroup
	wg.Add(procs)
	for p := 0; p < procs; p++ {
		lo := (n * p) / procs
		hi := (n * (p + 1)) / procs
		go func(id, lo, hi int) {
			defer wg.Done()
			s := data[lo:hi]
			mn, mx := s[0], s[0]
			asc, desc := true, true
			prev := s[0]
			for _, v := range s[1:] {
				if v < mn {
					mn = v
				} else if v > mx {
					mx = v
				}
				if v < prev {
					asc = false
				} else if v > prev {
					desc = false
				}
				prev = v
			}
			stats[id] = chunkStat{mn, mx, asc, desc}
		}(p, lo, hi)
	}
	wg.Wait()

	minVal, maxVal := stats[0].minV, stats[0].maxV
	asc, desc := stats[0].asc, stats[0].desc
	for p := 1; p < procs; p++ {
		if stats[p].minV < minVal {
			minVal = stats[p].minV
		}
		if stats[p].maxV > maxVal {
			maxVal = stats[p].maxV
		}
		boundary := (n * p) / procs
		if data[boundary] < data[boundary-1] {
			asc = false
		} else if data[boundary] > data[boundary-1] {
			desc = false
		}
		asc = asc && stats[p].asc
		desc = desc && stats[p].desc
	}
	if minVal == maxVal || asc {
		return
	}
	if desc {
		parallelReverse(data, procs)
		return
	}

	minU := uint64(minVal)
	span := uint64(maxVal) - minU
	bitLen := 64 - bits.LeadingZeros64(span)

	// Fast path: for small 8-bit spans (< 250K), single-threaded counting sort beats goroutine launch cost.
	if bitLen <= 8 && n < 250_000 {
		var counts [256]int
		for _, v := range data {
			counts[uint8(uint64(v)-minU)]++
		}
		out := 0
		limit := int(span)
		for r := 0; r <= limit; r++ {
			c := counts[r]
			if c > 0 {
				val := T(minU + uint64(r))
				for k := 0; k < c; k++ {
					data[out] = val
					out++
				}
			}
		}
		return
	}

	// Phase 2: Parallel 256-bucket histogram (allocated strictly for procs workers).
	localCounts := make([][256]int, procs)
	shift := uint(0)
	if bitLen > 8 {
		shift = uint(bitLen - 8)
	}
	wg.Add(procs)
	for p := 0; p < procs; p++ {
		lo := (n * p) / procs
		hi := (n * (p + 1)) / procs
		go func(id, lo, hi int) {
			defer wg.Done()
			c := &localCounts[id]
			if bitLen <= 8 {
				for _, v := range data[lo:hi] {
					c[uint8(uint64(v)-minU)]++
				}
			} else {
				for _, v := range data[lo:hi] {
					c[uint8((uint64(v)-minU)>>shift)]++
				}
			}
		}(p, lo, hi)
	}
	wg.Wait()

	var counts [256]int
	var offsets [256]int
	var writePos [256]int
	var ends [256]int
	var roomMask [4]uint64
	var activeList [256]uint8
	total := 0
	activeRooms := 0
	maxCount := 0

	for r := 0; r < 256; r++ {
		c := 0
		for p := 0; p < procs; p++ {
			c += localCounts[p][r]
		}
		if c > 0 {
			counts[r] = c
			roomMask[r>>6] |= uint64(1) << (uint(r) & 63)
			offsets[r] = total
			writePos[r] = total
			total += c
			ends[r] = total
			activeList[activeRooms] = uint8(r)
			activeRooms++
			if c > maxCount {
				maxCount = c
			}
		}
	}

	// Fast path: direct parallel counting-sort fill when span < 256.
	if bitLen <= 8 {
		var nextIdx atomic.Int32
		wg.Add(procs)
		for p := 0; p < procs; p++ {
			go func() {
				defer wg.Done()
				for {
					idx := int(nextIdx.Add(1) - 1)
					if idx >= activeRooms {
						return
					}
					r := activeList[idx]
					val := T(minU + uint64(r))
					sub := data[offsets[r]:ends[r]]
					for i := range sub {
						sub[i] = val
					}
				}
			}()
		}
		wg.Wait()
		return
	}

	// Adaptive parallel gate:
	// - Skewed/heavy-tailed/staircase data: median-of-65 branchless block-partition
	//   splits the bulk evenly across all cores instead of serializing a dominant room.
	if activeRooms < 32 || maxCount >= n/2 {
		depth := bits.Len(uint(procs)) + 1
		parallelSplitAndMaze(data, depth)
		return
	}

	// Spread-out data: cache-line block-buffered 256-way in-place scatter,
	// then dynamic work-stealing parallel sort across the 256 disjoint rooms.
	scatterBlockBuffered(data, minU, shift, &roomMask, &offsets, &writePos, &ends)

	var nextRoom atomic.Int32
	wg.Add(procs)
	for p := 0; p < procs; p++ {
		go func() {
			defer wg.Done()
			// Hoist L1 ping-pong buffer once per worker; large rooms use stack-allocated inPlaceMazeLarge.
			var stackAux [stackBufSize]T

			for {
				idx := int(nextRoom.Add(1) - 1)
				if idx >= activeRooms {
					return
				}
				r := activeList[idx]
				c := counts[r]
				if c <= 1 {
					continue
				}
				sub := data[offsets[r]:ends[r]]
				if c <= insertionLimit {
					insertionSort(sub)
				} else if c <= stackBufSize {
					stackMazePP(sub, stackAux[:c], false)
				} else {
					inPlaceMazeLarge(sub)
				}
			}
		}()
	}
	wg.Wait()
}

//go:noinline
func scatterBlockBuffered[T Integer](
	src []T,
	minU uint64,
	shift uint,
	roomMask *[4]uint64,
	offsets, writePos, ends *[256]int,
) {
	var buf [256][blockSize]T
	var pending [2048]T
	readPos := *offsets
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
}

func parallelSplitAndMaze[T Integer](data []T, depth int) {
	n := len(data)
	if depth <= 0 || n < parallelLeafLimit {
		Maze256(data)
		return
	}

	var samples [65]T
	step := n / 65
	for i := 0; i < 65; i++ {
		samples[i] = data[i*step]
	}
	slices.Sort(samples[:])
	pivot := samples[32]

	// Heavy duplicates around the median: 3-way partition completes the equal range immediately.
	if samples[16] == samples[48] {
		lt, gt := partition3Way(data, pivot)
		var wg sync.WaitGroup
		if lt > 1 && n-gt > 1 {
			wg.Add(1)
			go func() {
				defer wg.Done()
				parallelSplitAndMaze(data[:lt], depth-1)
			}()
			parallelSplitAndMaze(data[gt:], depth-1)
			wg.Wait()
		} else if lt > 1 {
			parallelSplitAndMaze(data[:lt], depth-1)
		} else if n-gt > 1 {
			parallelSplitAndMaze(data[gt:], depth-1)
		}
		return
	}

	mid := partitionBlockBranchless(data, pivot)
	if mid == 0 || mid == n {
		Maze256(data)
		return
	}

	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		parallelSplitAndMaze(data[:mid], depth-1)
	}()
	parallelSplitAndMaze(data[mid:], depth-1)
	wg.Wait()
}

// BlockQuicksort-style branchless 2-way partition: moves all elements < pivot before >= pivot.
func partitionBlockBranchless[T Integer](a []T, pivot T) int {
	const blk = 64
	var offL [blk]uint8
	var offR [blk]uint8

	l, r := 0, len(a)
	startL, numL := 0, 0
	startR, numR := 0, 0

	for r-l >= 2*blk {
		if numL == 0 {
			startL = 0
			s := a[l : l+blk]
			for i := 0; i < blk; i++ {
				offL[numL] = uint8(i)
				if s[i] >= pivot {
					numL++
				}
			}
		}
		if numR == 0 {
			startR = 0
			base := r - blk
			s := a[base:r]
			for i := 0; i < blk; i++ {
				offR[numR] = uint8(blk - 1 - i)
				if s[blk-1-i] < pivot {
					numR++
				}
			}
		}

		cnt := numL
		if numR < cnt {
			cnt = numR
		}
		if cnt > 0 {
			baseR := r - blk
			for k := 0; k < cnt; k++ {
				li := l + int(offL[startL+k])
				ri := baseR + int(offR[startR+k])
				a[li], a[ri] = a[ri], a[li]
			}
			startL += cnt
			startR += cnt
			numL -= cnt
			numR -= cnt
		}

		if numL == 0 {
			l += blk
		}
		if numR == 0 {
			r -= blk
		}
	}

	i, j := l, r-1
	for i <= j {
		for i <= j && a[i] < pivot {
			i++
		}
		for i <= j && a[j] >= pivot {
			j--
		}
		if i < j {
			a[i], a[j] = a[j], a[i]
			i++
			j--
		}
	}
	return i
}

func partition3Way[T Integer](a []T, pivot T) (int, int) {
	lt, i, gt := 0, 0, len(a)
	for i < gt {
		v := a[i]
		if v < pivot {
			a[lt], a[i] = v, a[lt]
			lt++
			i++
		} else if v > pivot {
			gt--
			a[i], a[gt] = a[gt], v
		} else {
			i++
		}
	}
	return lt, gt
}

func parallelReverse[T Integer](data []T, procs int) {
	n := len(data)
	half := n / 2
	var wg sync.WaitGroup
	wg.Add(procs)
	for p := 0; p < procs; p++ {
		lo := (half * p) / procs
		hi := (half * (p + 1)) / procs
		go func(lo, hi int) {
			defer wg.Done()
			for i := lo; i < hi; i++ {
				j := n - 1 - i
				data[i], data[j] = data[j], data[i]
			}
		}(lo, hi)
	}
	wg.Wait()
}
