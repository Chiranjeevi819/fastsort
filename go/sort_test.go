package fastsort

import (
	"fmt"
	"math"
	"math/rand"
	"slices"
	"testing"
)

type customInt64 int64

// ============================================================================
// CORRECTNESS TESTS (ALL 11 INTEGER TYPES + NAMED TYPE + ADVERSARIAL CASES)
// ============================================================================

func verifyType[T Integer](t *testing.T, name string, gen func(*rand.Rand, int) []T) {
	t.Helper()
	rng := rand.New(rand.NewSource(42))
	sizes := []int{0, 1, 2, 64, 65, 256, 257, 1024, 1025, 4096, 4097, 10000, 50000}
	for _, n := range sizes {
		data := gen(rng, n)
		expected := slices.Clone(data)
		slices.Sort(expected)

		got := slices.Clone(data)
		Sort(got)
		if !slices.Equal(got, expected) {
			t.Fatalf("Sort/%s (n=%d): sort mismatch", name, n)
		}

		gotPar := slices.Clone(data)
		SortParallel(gotPar)
		if !slices.Equal(gotPar, expected) {
			t.Fatalf("SortParallel/%s (n=%d): sort mismatch", name, n)
		}
	}
}

func TestSortAllIntegerTypes(t *testing.T) {
	verifyType(t, "int", func(r *rand.Rand, n int) []int {
		a := make([]int, n)
		for i := range a {
			a[i] = int(int64(r.Uint64()))
		}
		return a
	})
	verifyType(t, "int8", func(r *rand.Rand, n int) []int8 {
		a := make([]int8, n)
		for i := range a {
			a[i] = int8(r.Intn(256) - 128)
		}
		return a
	})
	verifyType(t, "int16", func(r *rand.Rand, n int) []int16 {
		a := make([]int16, n)
		for i := range a {
			a[i] = int16(r.Intn(65536) - 32768)
		}
		return a
	})
	verifyType(t, "int32", func(r *rand.Rand, n int) []int32 {
		a := make([]int32, n)
		for i := range a {
			a[i] = int32(r.Uint32())
		}
		return a
	})
	verifyType(t, "int64", func(r *rand.Rand, n int) []int64 {
		a := make([]int64, n)
		for i := range a {
			a[i] = int64(r.Uint64())
		}
		return a
	})
	verifyType(t, "uint", func(r *rand.Rand, n int) []uint {
		a := make([]uint, n)
		for i := range a {
			a[i] = uint(r.Uint64())
		}
		return a
	})
	verifyType(t, "uint8", func(r *rand.Rand, n int) []uint8 {
		a := make([]uint8, n)
		for i := range a {
			a[i] = uint8(r.Intn(256))
		}
		return a
	})
	verifyType(t, "uint16", func(r *rand.Rand, n int) []uint16 {
		a := make([]uint16, n)
		for i := range a {
			a[i] = uint16(r.Intn(65536))
		}
		return a
	})
	verifyType(t, "uint32", func(r *rand.Rand, n int) []uint32 {
		a := make([]uint32, n)
		for i := range a {
			a[i] = r.Uint32()
		}
		return a
	})
	verifyType(t, "uint64", func(r *rand.Rand, n int) []uint64 {
		a := make([]uint64, n)
		for i := range a {
			a[i] = r.Uint64()
		}
		return a
	})
	verifyType(t, "uintptr", func(r *rand.Rand, n int) []uintptr {
		a := make([]uintptr, n)
		for i := range a {
			a[i] = uintptr(r.Uint64())
		}
		return a
	})
	verifyType(t, "customInt64", func(r *rand.Rand, n int) []customInt64 {
		a := make([]customInt64, n)
		for i := range a {
			a[i] = customInt64(int64(r.Uint64()))
		}
		return a
	})
}

func TestSortEdgeCases(t *testing.T) {
	rng := rand.New(rand.NewSource(108))

	cases := []struct {
		name string
		data []int
	}{
		{"nil", nil},
		{"empty", []int{}},
		{"single", []int{42}},
		{"all_minInt64", makeFilled(50000, math.MinInt64)},
		{"all_maxInt64", makeFilled(50000, math.MaxInt64)},
		{"extreme_mix", makeExtremeMix(50000, rng)},
		{"span_255_cross0", makeRange(50000, -128, 127, rng)},
		{"span_256_cross0", makeRange(50000, -128, 128, rng)},
		{"pending_stress_65536", makePendingStress(65536)},
		{"reverse_buckets_50000", makeReverseBuckets(50000)},
		{"staircase_8b", makeStaircase8(50000, rng)},
		{"already_sorted", makeMonotonic(50000, true)},
		{"reverse_sorted", makeMonotonic(50000, false)},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			expected := slices.Clone(tc.data)
			slices.Sort(expected)

			got := slices.Clone(tc.data)
			Sort(got)
			if !slices.Equal(got, expected) {
				t.Fatalf("Sort mismatch on %s", tc.name)
			}

			gotPar := slices.Clone(tc.data)
			SortParallel(gotPar)
			if !slices.Equal(gotPar, expected) {
				t.Fatalf("SortParallel mismatch on %s", tc.name)
			}
		})
	}
}

func TestSortZeroAllocs(t *testing.T) {
	rng := rand.New(rand.NewSource(7))
	for _, n := range []int{100, 1000, 4096, 100_000} {
		template := makeUniform64(n, rng)
		work := make([]int, n)
		allocs := testing.AllocsPerRun(5, func() {
			copy(work, template)
			Sort(work)
		})
		if allocs != 0 {
			t.Fatalf("n=%d: expected 0 allocs/op, got %.1f", n, allocs)
		}
	}
}

// ============================================================================
// DATASET GENERATORS
// ============================================================================

func makeFilled(n, val int) []int {
	a := make([]int, n)
	for i := range a {
		a[i] = val
	}
	return a
}

func makeMonotonic(n int, asc bool) []int {
	a := make([]int, n)
	for i := range a {
		if asc {
			a[i] = i
		} else {
			a[i] = n - i
		}
	}
	return a
}

func makeExtremeMix(n int, rng *rand.Rand) []int {
	vals := []int{math.MinInt64, math.MinInt64 + 1, math.MinInt64 + 255, -1, 0, 1, math.MaxInt64 - 255, math.MaxInt64 - 1, math.MaxInt64}
	a := make([]int, n)
	for i := range a {
		a[i] = vals[rng.Intn(len(vals))]
	}
	return a
}

func makeRange(n, lo, hi int, rng *rand.Rand) []int {
	span := uint64(hi) - uint64(lo)
	a := make([]int, n)
	for i := range a {
		a[i] = int(uint64(lo) + rng.Uint64()%(span+1))
	}
	return a
}

func makeUniform64(n int, rng *rand.Rand) []int {
	a := make([]int, n)
	for i := range a {
		a[i] = int(int64(rng.Uint64()))
	}
	return a
}

func makePendingStress(n int) []int {
	a := make([]int, n)
	for i := 0; i < n; i++ {
		bucket := 255 - (i % 256)
		a[i] = (bucket << 20) | (i & 0xFFFFF)
	}
	return a
}

func makeReverseBuckets(n int) []int {
	a := make([]int, n)
	per := n / 256
	if per == 0 {
		per = 1
	}
	for i := 0; i < n; i++ {
		b := 255 - (i / per)
		if b < 0 {
			b = 0
		}
		a[i] = (b << 24) | ((n - i) & 0xFFFFFF)
	}
	return a
}

func makeStaircase8(n int, rng *rand.Rand) []int {
	a := make([]int, n)
	for i := range a {
		a[i] = rng.Intn(1000)
	}
	outliers := []int{1 << 61, 1 << 53, 1 << 45, 1 << 37, 1 << 29, 1 << 21, 1 << 13}
	for i, v := range outliers {
		if i < n {
			a[i] = v
		}
	}
	rng.Shuffle(len(a), func(i, j int) { a[i], a[j] = a[j], a[i] })
	return a
}

func makeDatasets(n int) []struct {
	name string
	data []int
} {
	rng := rand.New(rand.NewSource(108))

	u64 := makeUniform64(n, rng)

	u32 := make([]int, n)
	for i := range u32 {
		u32[i] = int(rng.Int31())
	}

	u10n := make([]int, n)
	for i := range u10n {
		u10n[i] = rng.Intn(n * 10)
	}

	sorted1Swap := makeMonotonic(n, true)
	if n >= 4 {
		sorted1Swap[n/2], sorted1Swap[n/2+1] = sorted1Swap[n/2+1], sorted1Swap[n/2]
	}

	nearlySorted := makeMonotonic(n, true)
	if n > 108 {
		for i := 0; i < n/50; i++ {
			idx := rng.Intn(n - 108)
			j := idx + rng.Intn(108)
			nearlySorted[idx], nearlySorted[j] = nearlySorted[j], nearlySorted[idx]
		}
	}

	reversed := makeMonotonic(n, false)

	pipeOrgan := make([]int, n)
	for i := 0; i < n/2; i++ {
		pipeOrgan[i] = i
	}
	for i := n / 2; i < n; i++ {
		pipeOrgan[i] = n - i
	}

	fewUnique := make([]int, n)
	for i := range fewUnique {
		fewUnique[i] = rng.Intn(16)
	}

	expOutliers := make([]int, n)
	for i := range expOutliers {
		if rng.Intn(100) == 0 {
			expOutliers[i] = rng.Intn(1_000_000) * 1_000_000
		} else {
			expOutliers[i] = rng.Intn(1000)
		}
	}

	logUniform := make([]int, n)
	for i := range logUniform {
		b := uint(rng.Intn(60))
		logUniform[i] = int((uint64(1) << b) | (rng.Uint64() & ((uint64(1) << b) - 1)))
	}

	staircase := makeStaircase8(n, rng)
	pending := makePendingStress(n)

	return []struct {
		name string
		data []int
	}{
		{"Uniform64", u64},
		{"Uniform32", u32},
		{"Random10N", u10n},
		{"Sorted1Swap", sorted1Swap},
		{"NearlySorted", nearlySorted},
		{"ReverseSorted", reversed},
		{"PipeOrgan", pipeOrgan},
		{"FewUnique16", fewUnique},
		{"ExpOutliers", expOutliers},
		{"LogUniform60", logUniform},
		{"Staircase8b", staircase},
		{"PendingStress", pending},
	}
}

// ============================================================================
// BENCHMARKS (testing.B WITH ns/elem AND -benchmem)
// ============================================================================

func benchPair[T Integer](b *testing.B, template []T) {
	n := len(template)
	work := make([]T, n)

	b.Run("Maze256", func(b *testing.B) {
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			b.StopTimer()
			copy(work, template)
			b.StartTimer()
			Maze256(work)
		}
		b.ReportMetric(float64(b.Elapsed().Nanoseconds())/float64(b.N*n), "ns/elem")
	})

	b.Run("Maze256Parallel", func(b *testing.B) {
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			b.StopTimer()
			copy(work, template)
			b.StartTimer()
			Maze256Parallel(work)
		}
		b.ReportMetric(float64(b.Elapsed().Nanoseconds())/float64(b.N*n), "ns/elem")
	})

	b.Run("slices.Sort", func(b *testing.B) {
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			b.StopTimer()
			copy(work, template)
			b.StartTimer()
			slices.Sort(work)
		}
		b.ReportMetric(float64(b.Elapsed().Nanoseconds())/float64(b.N*n), "ns/elem")
	})
}

func BenchmarkSizeScaling(b *testing.B) {
	rng := rand.New(rand.NewSource(999))
	for _, n := range []int{64, 256, 1_000, 10_000, 100_000, 1_000_000} {
		data := makeUniform64(n, rng)
		b.Run(fmt.Sprintf("N=%d", n), func(b *testing.B) {
			benchPair(b, data)
		})
	}
}

func BenchmarkDistributions1M(b *testing.B) {
	for _, ds := range makeDatasets(1_000_000) {
		b.Run(ds.name, func(b *testing.B) {
			benchPair(b, ds.data)
		})
	}
}

func BenchmarkGenericTypes100K(b *testing.B) {
	const n = 100_000
	rng := rand.New(rand.NewSource(123))

	u64 := make([]uint64, n)
	i32 := make([]int32, n)
	i16 := make([]int16, n)
	i8 := make([]int8, n)
	for i := 0; i < n; i++ {
		u64[i] = rng.Uint64()
		i32[i] = int32(rng.Uint32())
		i16[i] = int16(rng.Intn(65536) - 32768)
		i8[i] = int8(rng.Intn(256) - 128)
	}

	b.Run("uint64", func(b *testing.B) { benchPair(b, u64) })
	b.Run("int32", func(b *testing.B) { benchPair(b, i32) })
	b.Run("int16", func(b *testing.B) { benchPair(b, i16) })
	b.Run("int8", func(b *testing.B) { benchPair(b, i8) })
}
