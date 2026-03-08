//! Reduction operations (sum, min, max, mean, variance, normalize) on f32 slices.

#![allow(dead_code)]
#![allow(clippy::cast_precision_loss)]

/// Compute the sum of all elements in `data`.
///
/// Returns 0.0 for empty slices.
#[must_use]
pub fn sum_f32(data: &[f32]) -> f32 {
    data.iter().copied().fold(0.0f32, |acc, x| acc + x)
}

/// Return the minimum value in `data`.
///
/// Returns `f32::INFINITY` for empty slices.
pub fn min_f32(data: &[f32]) -> f32 {
    data.iter().copied().fold(f32::INFINITY, f32::min)
}

/// Return the maximum value in `data`.
///
/// Returns `f32::NEG_INFINITY` for empty slices.
pub fn max_f32(data: &[f32]) -> f32 {
    data.iter().copied().fold(f32::NEG_INFINITY, f32::max)
}

/// Compute the arithmetic mean of `data`.
///
/// Returns 0.0 for empty slices.
#[must_use]
pub fn mean_f32(data: &[f32]) -> f32 {
    if data.is_empty() {
        return 0.0;
    }
    sum_f32(data) / data.len() as f32
}

/// Compute the population variance of `data`.
///
/// Returns 0.0 for slices with fewer than 2 elements.
#[must_use]
pub fn variance_f32(data: &[f32]) -> f32 {
    if data.len() < 2 {
        return 0.0;
    }
    let mean = mean_f32(data);
    let sum_sq: f32 = data.iter().map(|&x| (x - mean) * (x - mean)).sum();
    sum_sq / data.len() as f32
}

/// Normalize `data` in-place to the range [0, 1].
///
/// If all values are equal (range is zero), all elements are set to 0.0.
pub fn normalize_f32(data: &mut [f32]) {
    if data.is_empty() {
        return;
    }
    let lo = min_f32(data);
    let hi = max_f32(data);
    let range = hi - lo;
    if range < f32::EPSILON {
        for x in data.iter_mut() {
            *x = 0.0;
        }
    } else {
        for x in data.iter_mut() {
            *x = (*x - lo) / range;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sum_basic() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        assert!((sum_f32(&data) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn test_sum_empty() {
        assert!((sum_f32(&[]) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_sum_single() {
        assert!((sum_f32(&[42.0]) - 42.0).abs() < 1e-5);
    }

    #[test]
    fn test_sum_negative() {
        let data = vec![-1.0f32, -2.0, 3.0];
        assert!((sum_f32(&data) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_min_basic() {
        let data = vec![3.0f32, 1.0, 4.0, 1.5, 9.0];
        assert!((min_f32(&data) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_min_empty() {
        assert!(min_f32(&[]).is_infinite());
    }

    #[test]
    fn test_min_all_negative() {
        let data = vec![-5.0f32, -1.0, -3.0];
        assert!((min_f32(&data) - (-5.0)).abs() < 1e-5);
    }

    #[test]
    fn test_max_basic() {
        let data = vec![3.0f32, 1.0, 7.0, 2.0];
        assert!((max_f32(&data) - 7.0).abs() < 1e-5);
    }

    #[test]
    fn test_max_empty() {
        assert!(max_f32(&[]).is_infinite());
    }

    #[test]
    fn test_max_single() {
        assert!((max_f32(&[99.0]) - 99.0).abs() < 1e-5);
    }

    #[test]
    fn test_mean_basic() {
        let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        assert!((mean_f32(&data) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_mean_empty() {
        assert!((mean_f32(&[]) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_mean_single() {
        assert!((mean_f32(&[7.0]) - 7.0).abs() < 1e-5);
    }

    #[test]
    fn test_variance_basic() {
        // [2, 4, 4, 4, 5, 5, 7, 9] mean=5, variance=4
        let data = vec![2.0f32, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let v = variance_f32(&data);
        assert!((v - 4.0).abs() < 1e-4, "variance: {v}");
    }

    #[test]
    fn test_variance_single() {
        assert!((variance_f32(&[5.0]) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_variance_empty() {
        assert!((variance_f32(&[]) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_variance_uniform() {
        let data = vec![3.0f32, 3.0, 3.0, 3.0];
        assert!((variance_f32(&data) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_basic() {
        let mut data = vec![0.0f32, 5.0, 10.0];
        normalize_f32(&mut data);
        assert!((data[0] - 0.0).abs() < 1e-5);
        assert!((data[1] - 0.5).abs() < 1e-5);
        assert!((data[2] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_normalize_uniform() {
        let mut data = vec![4.0f32, 4.0, 4.0];
        normalize_f32(&mut data);
        for &v in &data {
            assert!((v - 0.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_normalize_empty() {
        let mut data: Vec<f32> = Vec::new();
        normalize_f32(&mut data); // Should not panic
    }

    #[test]
    fn test_normalize_negative_range() {
        let mut data = vec![-10.0f32, 0.0, 10.0];
        normalize_f32(&mut data);
        assert!((data[0] - 0.0).abs() < 1e-5);
        assert!((data[1] - 0.5).abs() < 1e-5);
        assert!((data[2] - 1.0).abs() < 1e-5);
    }
}
