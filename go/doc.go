// Package mazesort provides high-performance, zero-allocation in-place sorting
// algorithms for Go.
//
// Its flagship integer sorting algorithm is In-Place Maze256, available in both
// single-threaded ([Sort] / [Maze256], strictly 0 B/op and 0 allocs/op) and
// multi-core parallel ([SortParallel] / [Maze256Parallel], zero O(n) auxiliary
// slice allocations) variants.
package mazesort
