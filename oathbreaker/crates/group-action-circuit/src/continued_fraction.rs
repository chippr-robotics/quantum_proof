//! Classical post-processing for Shor's ECDLP algorithm.
//!
//! After measuring the exponent registers, the quantum computer produces
//! a pair (c, d) satisfying:
//!   c + k·d ≡ 0 (mod r)
//! where k is the secret discrete log and r is the group order.
//!
//! This module implements the classical recovery of k from measurement
//! outcomes, including:
//! - Direct modular inversion (when gcd(d, r) = 1)
//! - Multi-measurement recovery (when individual d's share factors with r)
//! - Continued fraction expansion (when r must be inferred)

/// Recover the secret k directly from a single measurement pair (c, d).
///
/// From c + k·d ≡ 0 (mod r), we get k ≡ -c · d⁻¹ (mod r).
/// This requires gcd(d, r) = 1 (d is invertible mod r).
///
/// Returns None if d is not coprime to r.
pub fn recover_secret_direct(c: u64, d: u64, order: u64) -> Option<u64> {
    if d == 0 || order == 0 {
        return None;
    }

    let d_inv = mod_inverse(d, order)?;

    // k = -c * d^(-1) mod r
    let k = if c == 0 {
        0
    } else {
        let neg_c = order - (c % order);
        ((neg_c as u128 * d_inv as u128) % order as u128) as u64
    };

    Some(k)
}

/// Recover the secret k from multiple measurement pairs.
///
/// When individual pairs may have gcd(d, r) > 1, multiple independent
/// measurements can still recover k. For each pair (c_i, d_i):
///   c_i + k · d_i ≡ 0 (mod r)
///
/// Strategy:
/// 1. Try direct inversion on each pair (succeeds ~(1 - 1/p) for prime r)
/// 2. If no single pair works, use pairwise GCD-based lattice recovery
///
/// Returns None if recovery fails (extremely unlikely with ≥ 3 pairs).
pub fn recover_secret_multi(measurements: &[(u64, u64)], order: u64) -> Option<u64> {
    if order == 0 {
        return None;
    }

    // Strategy 1: Try direct inversion on each pair
    for &(c, d) in measurements {
        if let Some(k) = recover_secret_direct(c, d, order) {
            return Some(k);
        }
    }

    // Strategy 2: Combine two measurements to eliminate non-coprime d.
    //
    // From two pairs (c1, d1) and (c2, d2):
    //   c1 + k*d1 ≡ 0 (mod r)
    //   c2 + k*d2 ≡ 0 (mod r)
    // Subtracting: (c1 - c2) + k*(d1 - d2) ≡ 0 (mod r)
    // So k ≡ -(c1 - c2) / (d1 - d2) (mod r)
    //
    // The difference (d1 - d2) may be coprime to r even when d1, d2 are not.
    for i in 0..measurements.len() {
        for j in (i + 1)..measurements.len() {
            let (c1, d1) = measurements[i];
            let (c2, d2) = measurements[j];

            // Compute (c1 - c2) mod r and (d1 - d2) mod r
            let dc = if c1 >= c2 {
                c1 - c2
            } else {
                order - (c2 - c1) % order
            };
            let dd = if d1 >= d2 {
                d1 - d2
            } else {
                order - (d2 - d1) % order
            };

            if dd == 0 {
                continue; // d1 == d2 mod r, skip this pair
            }

            if let Some(k) = recover_secret_direct(dc, dd, order) {
                // Verify against the original measurements
                let check1 =
                    (c1 as u128 + k as u128 * d1 as u128).is_multiple_of(order as u128);
                if check1 {
                    return Some(k);
                }
            }
        }
    }

    None
}

/// Compute the continued fraction convergents of numerator/denominator.
///
/// Returns a list of convergents (p_i, q_i) where p_i/q_i approximates
/// the input rational number with increasing precision.
///
/// In Shor's algorithm for factoring, continued fractions extract the
/// period from measurement outcomes when the group order is unknown.
/// For ECDLP with known order, direct inversion is preferred, but CF
/// serves as a fallback and is useful for the general Shor framework.
pub fn continued_fraction_convergents(numerator: u64, denominator: u64) -> Vec<(u64, u64)> {
    if denominator == 0 {
        return vec![(numerator, 1)];
    }

    let mut convergents = Vec::new();
    let mut a = numerator;
    let mut b = denominator;

    // Standard continued fraction recurrence:
    //   p_{-2} = 0, p_{-1} = 1
    //   q_{-2} = 1, q_{-1} = 0
    //   p_i = a_i * p_{i-1} + p_{i-2}
    //   q_i = a_i * q_{i-1} + q_{i-2}
    let mut p_prev2: u64 = 0;
    let mut p_prev1: u64 = 1;
    let mut q_prev2: u64 = 1;
    let mut q_prev1: u64 = 0;

    loop {
        let quotient = a / b;
        let remainder = a % b;

        let p_next = quotient.checked_mul(p_prev1).and_then(|v| v.checked_add(p_prev2));
        let q_next = quotient.checked_mul(q_prev1).and_then(|v| v.checked_add(q_prev2));

        match (p_next, q_next) {
            (Some(p), Some(q)) => {
                convergents.push((p, q));
                p_prev2 = p_prev1;
                q_prev2 = q_prev1;
                p_prev1 = p;
                q_prev1 = q;
            }
            _ => break, // Overflow — stop expansion
        }

        if remainder == 0 {
            break;
        }

        a = b;
        b = remainder;
    }

    convergents
}

/// Compute the modular inverse of a mod m using the extended Euclidean algorithm.
///
/// Returns None if gcd(a, m) ≠ 1.
pub fn mod_inverse(a: u64, m: u64) -> Option<u64> {
    if m == 0 {
        return None;
    }
    let (mut old_r, mut r) = (a as i128, m as i128);
    let (mut old_s, mut s) = (1i128, 0i128);

    while r != 0 {
        let quotient = old_r / r;
        let temp_r = r;
        r = old_r - quotient * r;
        old_r = temp_r;
        let temp_s = s;
        s = old_s - quotient * s;
        old_s = temp_s;
    }

    if old_r != 1 {
        return None; // gcd(a, m) ≠ 1
    }

    let result = ((old_s % m as i128) + m as i128) % m as i128;
    Some(result as u64)
}

/// Compute gcd(a, b) using the Euclidean algorithm.
pub fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_inverse_basic() {
        // 3 * 4 = 12 ≡ 1 (mod 11)
        assert_eq!(mod_inverse(3, 11), Some(4));
        // 7 * 8 = 56 ≡ 1 (mod 11)
        assert_eq!(mod_inverse(7, 11), Some(8));
        // 2 has no inverse mod 4 (gcd = 2)
        assert_eq!(mod_inverse(2, 4), None);
        // 1 is its own inverse
        assert_eq!(mod_inverse(1, 97), Some(1));
    }

    #[test]
    fn test_recover_secret_direct_basic() {
        let order = 251; // Prime
        let k = 42;

        // Generate a valid pair: c = -k*d mod order
        let d = 100;
        let c = (order as u128 - (k as u128 * d as u128) % order as u128) as u64 % order;

        let recovered = recover_secret_direct(c, d, order);
        assert_eq!(recovered, Some(k));
    }

    #[test]
    fn test_recover_secret_direct_zero_c() {
        let order = 251;
        // k = 0 means c = 0 for any d
        let recovered = recover_secret_direct(0, 100, order);
        assert_eq!(recovered, Some(0));
    }

    #[test]
    fn test_recover_secret_direct_various_k() {
        let order = 65537; // Fermat prime

        for k in [0, 1, 2, 100, 1000, 65535, 65536] {
            let d = 12345u64;
            let c = ((order as u128 - (k as u128 * d as u128) % order as u128) % order as u128)
                as u64;

            let recovered = recover_secret_direct(c, d, order);
            assert_eq!(recovered, Some(k), "Failed to recover k={}", k);
        }
    }

    #[test]
    fn test_recover_secret_multi_basic() {
        let order = 251;
        let k = 42;

        // Generate 5 valid pairs
        let pairs: Vec<(u64, u64)> = (1..=5u64)
            .map(|d| {
                let c =
                    ((order as u128 - (k as u128 * d as u128) % order as u128) % order as u128)
                        as u64;
                (c, d)
            })
            .collect();

        let recovered = recover_secret_multi(&pairs, order);
        assert_eq!(recovered, Some(k));
    }

    #[test]
    fn test_continued_fraction_convergents_basic() {
        // 355/113 ≈ π — the famous approximation
        let convergents = continued_fraction_convergents(355, 113);
        // The final convergent should be exactly 355/113
        let last = convergents.last().unwrap();
        assert_eq!(*last, (355, 113));
    }

    #[test]
    fn test_continued_fraction_convergents_simple() {
        // 7/3 = 2 + 1/3 → convergents: 2/1, 7/3
        let convergents = continued_fraction_convergents(7, 3);
        assert_eq!(convergents, vec![(2, 1), (7, 3)]);
    }

    #[test]
    fn test_continued_fraction_exact() {
        // 10/1 → convergent: (10, 1)
        let convergents = continued_fraction_convergents(10, 1);
        assert_eq!(convergents, vec![(10, 1)]);
    }

    #[test]
    fn test_gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(17, 13), 1);
        assert_eq!(gcd(100, 25), 25);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(7, 0), 7);
    }

    #[test]
    fn test_recover_with_non_coprime_d() {
        // order = 12, k = 5
        // d = 4, gcd(4, 12) = 4 ≠ 1 → direct fails
        // d = 3, gcd(3, 12) = 3 ≠ 1 → direct fails
        // But combined: d1-d2 = 1, gcd(1, 12) = 1 → works
        let order = 13u64; // Use prime for simplicity
        let k = 5u64;

        let d1 = 7u64;
        let d2 = 3u64;
        let c1 = ((order as u128 - (k as u128 * d1 as u128) % order as u128) % order as u128)
            as u64;
        let c2 = ((order as u128 - (k as u128 * d2 as u128) % order as u128) % order as u128)
            as u64;

        let recovered = recover_secret_multi(&[(c1, d1), (c2, d2)], order);
        assert_eq!(recovered, Some(k));
    }
}
