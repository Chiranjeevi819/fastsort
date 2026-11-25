package mazesort

// Integer constrains integer sorting algorithms to all built-in signed and
// unsigned integer types, as well as user-defined types with those underlying types.
type Integer interface {
	~int | ~int8 | ~int16 | ~int32 | ~int64 |
		~uint | ~uint8 | ~uint16 | ~uint32 | ~uint64 | ~uintptr
}

// Sort sorts data in ascending order in-place using the package's default
// integer sorting algorithm ([Maze256]), with O(n) expected time and O(1)
// heap space (0 allocs/op).
func Sort[T Integer](data []T) {
	Maze256(data)
}

// SortParallel sorts data in ascending order in-place across multiple CPU cores
// using the package's default parallel integer sorting algorithm
// ([Maze256Parallel]), with O(1) auxiliary data memory.
func SortParallel[T Integer](data []T) {
	Maze256Parallel(data)
}

func handleMonotonic[T Integer](data []T) bool {
	n := len(data)
	asc, desc := true, true
	for i := 1; i < n; i++ {
		if data[i] < data[i-1] {
			asc = false
		}
		if data[i] > data[i-1] {
			desc = false
		}
		if !asc && !desc {
			return false
		}
	}
	if desc {
		for l, r := 0, n-1; l < r; l, r = l+1, r-1 {
			data[l], data[r] = data[r], data[l]
		}
	}
	return true
}

func insertionSort[T Integer](a []T) {
	for i := 1; i < len(a); i++ {
		v := a[i]
		if a[i-1] > v {
			j := i
			for j > 0 && a[j-1] > v {
				a[j] = a[j-1]
				j--
			}
			a[j] = v
		}
	}
}
