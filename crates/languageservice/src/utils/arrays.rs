//! Port of `src/utils/arrays.ts`.

/// Takes a sorted array and a predicate `p`. The array is sorted in such a way
/// that all elements where `p(x)` is false are located before all elements where
/// `p(x)` is true.
///
/// Returns the least index `x` for which `p(x)` is true, or `array.len()` if no
/// element satisfies the predicate.
pub fn find_first<T>(array: &[T], p: impl Fn(&T) -> bool) -> usize {
    let mut low = 0usize;
    let mut high = array.len();
    if high == 0 {
        return 0;
    }
    while low < high {
        let mid = (low + high) / 2;
        if p(&array[mid]) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

/// Port of `binarySearch`. Returns the index of `key`, or `-(insertion_point + 1)`.
pub fn binary_search<T>(array: &[T], key: &T, comparator: impl Fn(&T, &T) -> i32) -> isize {
    let mut low = 0isize;
    let mut high = array.len() as isize - 1;

    while low <= high {
        let mid = (low + high) / 2;
        let comp = comparator(&array[mid as usize], key);
        if comp < 0 {
            low = mid + 1;
        } else if comp > 0 {
            high = mid - 1;
        } else {
            return mid;
        }
    }
    -(low + 1)
}
