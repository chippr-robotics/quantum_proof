//! Shor's ECDLP measurement outcome simulation.
//!
//! A real quantum computer running Shor's ECDLP algorithm would:
//! 1. Prepare a uniform superposition over both exponent registers
//! 2. Apply the group-action map coherently
//! 3. Apply inverse QFT to both exponent registers
//! 4. Measure both registers, obtaining (c, d)
//!
//! The measurement outcome (c, d) satisfies the relation:
//!   c + k·d ≡ 0 (mod r)
//! where k is the secret discrete log and r is the group order.
//!
//! Since full quantum state simulation requires O(2^n) memory (infeasible
//! for n = 64), we instead generate mathematically valid measurement
//! outcomes that a quantum computer would produce. This allows testing
//! the classical post-processing pipeline without exponential resources.

/// Generate a single valid Shor measurement pair (c, d) for a known secret k.
///
/// The pair satisfies c + k*d ≡ 0 (mod order), which is the relation
/// that a quantum measurement would produce.
///
/// For the ECDLP dual-register formulation:
/// - d is chosen uniformly at random from [1, order)
/// - c is computed as c = (-k * d) mod order
///
/// In practice, ~60% of measurements yield d coprime to order
/// (when order is prime, 100% of d ≠ 0 are coprime), enabling
/// direct recovery of k via modular inversion.
pub fn sample_measurement_pair(k: u64, order: u64, seed: u64) -> (u64, u64) {
    // Use a simple deterministic PRNG seeded by `seed` to generate d.
    // This makes tests reproducible while simulating the randomness
    // of quantum measurement.
    let d = deterministic_nonzero_mod(seed, order);
    let c = compute_c(k, d, order);
    (c, d)
}

/// Generate multiple valid measurement pairs for testing.
///
/// Each pair independently satisfies c + k*d ≡ 0 (mod order).
/// Multiple pairs can be combined for lattice-based recovery when
/// individual pairs have gcd(d, order) > 1.
pub fn sample_measurement_pairs(k: u64, order: u64, count: usize) -> Vec<(u64, u64)> {
    (0..count)
        .map(|i| sample_measurement_pair(k, order, i as u64 + 1))
        .collect()
}

/// Verify that a measurement pair satisfies the Shor relation.
///
/// Returns true if c + k*d ≡ 0 (mod order).
pub fn verify_measurement_pair(c: u64, d: u64, k: u64, order: u64) -> bool {
    let lhs = ((c as u128) + (k as u128) * (d as u128)) % (order as u128);
    lhs == 0
}

/// Compute c = (-k * d) mod order.
fn compute_c(k: u64, d: u64, order: u64) -> u64 {
    let kd = (k as u128 * d as u128) % order as u128;
    if kd == 0 {
        0
    } else {
        (order as u128 - kd) as u64
    }
}

/// Deterministic non-zero value mod m, derived from seed.
///
/// Uses a simple hash-like mixing function. Not cryptographically secure,
/// but sufficient for deterministic test generation.
fn deterministic_nonzero_mod(seed: u64, m: u64) -> u64 {
    if m <= 1 {
        return 1;
    }
    // SplitMix64-style mixing for good distribution
    let mut z = seed.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^= z >> 31;

    // Map to [1, m-1]
    (z % (m - 1)) + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measurement_pair_satisfies_relation() {
        let order = 251; // Small prime order
        let k = 42;

        for seed in 1..=100 {
            let (c, d) = sample_measurement_pair(k, order, seed);
            assert!(
                verify_measurement_pair(c, d, k, order),
                "Pair ({}, {}) should satisfy c + k*d ≡ 0 (mod {})",
                c,
                d,
                order
            );
        }
    }

    #[test]
    fn test_measurement_pairs_batch() {
        let order = 65537; // Fermat prime
        let k = 12345;
        let pairs = sample_measurement_pairs(k, order, 50);

        assert_eq!(pairs.len(), 50);
        for (c, d) in &pairs {
            assert!(verify_measurement_pair(*c, *d, k, order));
        }
    }

    #[test]
    fn test_measurement_d_nonzero() {
        let order = 1009;
        let k = 7;

        for seed in 1..=200 {
            let (_, d) = sample_measurement_pair(k, order, seed);
            assert!(d > 0, "d must be non-zero");
            assert!(d < order, "d must be less than order");
        }
    }

    #[test]
    fn test_measurement_c_in_range() {
        let order = 997;
        let k = 500;

        for seed in 1..=100 {
            let (c, _) = sample_measurement_pair(k, order, seed);
            assert!(c < order, "c must be less than order");
        }
    }
}
