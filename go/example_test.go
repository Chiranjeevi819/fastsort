package fastsort_test

import (
	"fmt"

	fastsort "github.com/Chiranjeevi819/fastsort/go"
)

func ExampleSort() {
	nums := []int64{42, -7, 1000, 0, -999, 13}
	fastsort.Sort(nums)
	fmt.Println(nums)
	// Output: [-999 -7 0 13 42 1000]
}

func ExampleMaze256() {
	ids := []uint32{500, 12, 99999, 1, 42}
	fastsort.Maze256(ids)
	fmt.Println(ids)
	// Output: [1 12 42 500 99999]
}

func ExampleMaze256Parallel() {
	vals := []int{99, -5, 0, 128, -128, 7}
	fastsort.Maze256Parallel(vals)
	fmt.Println(vals)
	// Output: [-128 -5 0 7 99 128]
}
