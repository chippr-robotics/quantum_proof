//! Windowed Non-Adjacent Form (wNAF) scalar encoding for elliptic curve
//! scalar multiplication.
//!
//! wNAF encodes a scalar using digits from {-(2^(w-1)-1), ..., -1, 0, 1, ..., 2^(w-1)-1}
//! with the property that no two adjacent digits are both non-zero. This means
//! approximately 1/(w+1) of the digits are non-zero, reducing the number of
//! point additions needed during scalar multiplication.
//!
//! For w=2 (standard NAF): digits in {-1, 0, 1}, ~1/3 non-zero.
//! For w=3: digits in {-3, -1, 0, 1, 3}, ~1/4 non-zero.
//! For w=4: digits in {-7, ..., -1, 0, 1, ..., 7}, ~1/5 non-zero.
//!
//! In the quantum circuit context, zero digits allow skipping the point addition
//! entirely (just perform the doubling), and negative digits use point negation
//! which is free in Jacobian coordinates (negate Y only, no field multiplication).

/// A single wNAF digit: either zero or an odd value in [-(2^(w-1)-1), 2^(w-1)-1].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WnafDigit {
    Zero,
    NonZero(i32),
}

impl WnafDigit {
    pub fn is_zero(&self) -> bool {
        matches!(self, WnafDigit::Zero)
    }

    /// Returns the absolute value and sign for table lookup.
    /// For NonZero(d): table index = (|d| - 1) / 2, negate = d < 0.
    pub fn table_index_and_sign(&self) -> Option<(usize, bool)> {
        match self {
            WnafDigit::Zero => None,
            WnafDigit::NonZero(d) => {
                let abs_d = d.unsigned_abs() as usize;
                let idx = (abs_d - 1) / 2; // odd values 1,3,5,... map to 0,1,2,...
                Some((idx, *d < 0))
            }
        }
    }
}

/// Compute the width-w Non-Adjacent Form of a scalar.
///
/// The wNAF representation has the following properties:
/// 1. Every non-zero digit is odd
/// 2. At most one of any w consecutive digits is non-zero
/// 3. The most significant non-zero digit is positive
/// 4. The representation is at most one digit longer than the binary representation
///
/// # Arguments
/// * `scalar` - The scalar value to encode
/// * `w` - The window width (must be >= 2)
///
/// # Returns
/// A vector of WnafDigit, least significant digit first.
pub fn compute_wnaf(scalar: u64, w: u32) -> Vec<WnafDigit> {
    assert!(w >= 2, "wNAF window width must be >= 2");
    let half_window = 1i64 << (w - 1); // 2^(w-1)
    let full_window = 1i64 << w; // 2^w
    let mask = full_window - 1; // 2^w - 1

    let mut digits = Vec::new();
    let mut k = scalar as i128;

    while k > 0 {
        if k & 1 != 0 {
            // k is odd: extract a w-bit window
            let mut d = (k & mask as i128) as i64;
            if d >= half_window {
                d -= full_window;
            }
            digits.push(WnafDigit::NonZero(d as i32));
            k -= d as i128;
        } else {
            digits.push(WnafDigit::Zero);
        }
        k >>= 1;
    }

    digits
}

/// Compute the standard (width-2) Non-Adjacent Form.
///
/// This is the simplest NAF where digits are in {-1, 0, 1} with no two
/// adjacent non-zero digits.
pub fn compute_naf(scalar: u64) -> Vec<WnafDigit> {
    compute_wnaf(scalar, 2)
}

/// Count the number of non-zero digits in a wNAF representation.
pub fn wnaf_nonzero_count(digits: &[WnafDigit]) -> usize {
    digits.iter().filter(|d| !d.is_zero()).count()
}

/// Reconstruct the scalar value from its wNAF representation.
///
/// Used for verification: wnaf_to_scalar(compute_wnaf(k, w)) == k.
pub fn wnaf_to_scalar(digits: &[WnafDigit]) -> u64 {
    let mut value: i128 = 0;
    for (i, digit) in digits.iter().enumerate() {
        if let WnafDigit::NonZero(d) = digit {
            value += (*d as i128) << i;
        }
    }
    value as u64
}

/// Compute the precomputation table size needed for width-w wNAF.
///
/// The table stores points 1\*P, 3\*P, 5\*P, ..., (2^(w-1)-1)\*P.
/// Size = 2^(w-2) entries (for w >= 2).
pub fn wnaf_table_size(w: u32) -> usize {
    if w < 2 {
        1
    } else {
        1usize << (w - 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_naf_basic_values() {
        // 0 → empty
        assert!(compute_naf(0).is_empty());

        // 1 → [1]
        let naf = compute_naf(1);
        assert_eq!(naf, vec![WnafDigit::NonZero(1)]);

        // 2 → [0, 1]
        let naf = compute_naf(2);
        assert_eq!(naf, vec![WnafDigit::Zero, WnafDigit::NonZero(1)]);

        // 3 → [−1, 0, 1] (since 3 = 4 - 1)
        let naf = compute_naf(3);
        assert_eq!(
            naf,
            vec![
                WnafDigit::NonZero(-1),
                WnafDigit::Zero,
                WnafDigit::NonZero(1)
            ]
        );

        // 7 → [−1, 0, 0, 1] (since 7 = 8 - 1)
        let naf = compute_naf(7);
        assert_eq!(
            naf,
            vec![
                WnafDigit::NonZero(-1),
                WnafDigit::Zero,
                WnafDigit::Zero,
                WnafDigit::NonZero(1)
            ]
        );
    }

    #[test]
    fn test_naf_no_adjacent_nonzero() {
        // For many test values, verify no two adjacent digits are non-zero
        for k in 0..1000 {
            let naf = compute_naf(k);
            for pair in naf.windows(2) {
                assert!(
                    pair[0].is_zero() || pair[1].is_zero(),
                    "Adjacent non-zero digits in NAF of {}: {:?}",
                    k,
                    naf,
                );
            }
        }
    }

    #[test]
    fn test_naf_roundtrip() {
        // Verify wnaf_to_scalar(compute_naf(k)) == k for many values
        for k in 0..1000 {
            let naf = compute_naf(k);
            let reconstructed = wnaf_to_scalar(&naf);
            assert_eq!(
                reconstructed, k,
                "NAF roundtrip failed for k={}: {:?}",
                k, naf,
            );
        }
    }

    #[test]
    fn test_naf_roundtrip_large() {
        let values = [
            u64::MAX,
            u64::MAX - 1,
            0x8000_0000_0000_0000,
            0xDEAD_BEEF_CAFE_BABE,
            0x0123_4567_89AB_CDEF,
        ];
        for &k in &values {
            let naf = compute_naf(k);
            let reconstructed = wnaf_to_scalar(&naf);
            assert_eq!(reconstructed, k, "NAF roundtrip failed for k={:#x}", k);
        }
    }

    #[test]
    fn test_naf_fewer_nonzero_than_binary() {
        // NAF should have fewer non-zero digits than binary on average
        let mut binary_total = 0usize;
        let mut naf_total = 0usize;
        for k in 1u64..1000 {
            binary_total += k.count_ones() as usize;
            naf_total += wnaf_nonzero_count(&compute_naf(k));
        }
        assert!(
            naf_total < binary_total,
            "NAF should have fewer non-zero digits: naf={} vs binary={}",
            naf_total,
            binary_total,
        );
    }

    #[test]
    fn test_wnaf_w3_roundtrip() {
        for k in 0..1000 {
            let wnaf = compute_wnaf(k, 3);
            let reconstructed = wnaf_to_scalar(&wnaf);
            assert_eq!(reconstructed, k, "wNAF(w=3) roundtrip failed for k={}", k);
        }
    }

    #[test]
    fn test_wnaf_w4_roundtrip() {
        for k in 0..1000 {
            let wnaf = compute_wnaf(k, 4);
            let reconstructed = wnaf_to_scalar(&wnaf);
            assert_eq!(reconstructed, k, "wNAF(w=4) roundtrip failed for k={}", k);
        }
    }

    #[test]
    fn test_wnaf_w3_non_adjacency() {
        // For w=3, at most one of any 3 consecutive digits is non-zero
        for k in 0..1000 {
            let wnaf = compute_wnaf(k, 3);
            for window in wnaf.windows(3) {
                let nz = window.iter().filter(|d| !d.is_zero()).count();
                assert!(
                    nz <= 1,
                    "More than 1 non-zero in 3-window for k={}: {:?}",
                    k,
                    wnaf,
                );
            }
        }
    }

    #[test]
    fn test_wnaf_digits_are_odd() {
        // All non-zero wNAF digits must be odd
        for w in 2..=5 {
            for k in 1u64..500 {
                let wnaf = compute_wnaf(k, w);
                for digit in &wnaf {
                    if let WnafDigit::NonZero(d) = digit {
                        assert!(
                            d.unsigned_abs() % 2 == 1,
                            "wNAF digit {} is even for k={}, w={}",
                            d,
                            k,
                            w,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_wnaf_digit_range() {
        // Non-zero digits must be in [-(2^(w-1)-1), 2^(w-1)-1]
        for w in 2u32..=5 {
            let bound = (1i32 << (w - 1)) - 1;
            for k in 1u64..500 {
                let wnaf = compute_wnaf(k, w);
                for digit in &wnaf {
                    if let WnafDigit::NonZero(d) = digit {
                        assert!(
                            d.abs() <= bound,
                            "wNAF digit {} out of range [-{}, {}] for k={}, w={}",
                            d,
                            bound,
                            bound,
                            k,
                            w,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_wnaf_table_size() {
        assert_eq!(wnaf_table_size(2), 1); // NAF: only [1]P
        assert_eq!(wnaf_table_size(3), 2); // [1]P, [3]P
        assert_eq!(wnaf_table_size(4), 4); // [1]P, [3]P, [5]P, [7]P
        assert_eq!(wnaf_table_size(5), 8);
    }

    #[test]
    fn test_table_index_and_sign() {
        let d = WnafDigit::NonZero(1);
        assert_eq!(d.table_index_and_sign(), Some((0, false)));

        let d = WnafDigit::NonZero(-1);
        assert_eq!(d.table_index_and_sign(), Some((0, true)));

        let d = WnafDigit::NonZero(3);
        assert_eq!(d.table_index_and_sign(), Some((1, false)));

        let d = WnafDigit::NonZero(-5);
        assert_eq!(d.table_index_and_sign(), Some((2, true)));

        let d = WnafDigit::Zero;
        assert_eq!(d.table_index_and_sign(), None);
    }

    #[test]
    fn test_wnaf_higher_w_fewer_nonzero() {
        // Higher w should produce fewer non-zero digits (for large enough scalars)
        let k = 0xDEAD_BEEF_CAFE_BABEu64;
        let nz2 = wnaf_nonzero_count(&compute_wnaf(k, 2));
        let nz3 = wnaf_nonzero_count(&compute_wnaf(k, 3));
        let nz4 = wnaf_nonzero_count(&compute_wnaf(k, 4));
        assert!(
            nz3 <= nz2,
            "w=3 should have <= non-zero digits than w=2: {} vs {}",
            nz3,
            nz2,
        );
        assert!(
            nz4 <= nz3,
            "w=4 should have <= non-zero digits than w=3: {} vs {}",
            nz4,
            nz3,
        );
    }
}
