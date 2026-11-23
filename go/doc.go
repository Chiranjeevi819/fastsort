// Package fastsort provides high-performance, in-place sorting algorithms for Go.
//
// Its flagship integer sorting algorithm is In-Place Maze256, available in both
// single-threaded ([Maze256] / [Sort], strictly 0 B/op and 0 allocs/op) and
// multi-core parallel ([Maze256Parallel] / [SortParallel], zero O(n) auxiliary
// slice allocations) variants.
package fastsort
