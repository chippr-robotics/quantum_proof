/// The Goldilocks prime: p = 2^64 - 2^32 + 1
pub const GOLDILOCKS_PRIME: u64 = 0xFFFF_FFFF_0000_0001;

/// p - 1, useful for Fermat's little theorem exponent computation
pub const P_MINUS_ONE: u64 = GOLDILOCKS_PRIME - 1;

/// p - 2, the exponent for computing multiplicative inverses via Fermat
pub const P_MINUS_TWO: u64 = GOLDILOCKS_PRIME - 2;

/// (p - 1) / 2, the exponent for computing the Legendre symbol
pub const P_MINUS_ONE_HALF: u64 = P_MINUS_ONE / 2;

/// A known multiplicative generator of GF(p)*
/// (to be validated by the curve generation scripts)
pub const MULTIPLICATIVE_GENERATOR: u64 = 7;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prime_form() {
        // Verify p = 2^64 - 2^32 + 1
        let expected: u128 = (1u128 << 64) - (1u128 << 32) + 1;
        assert_eq!(GOLDILOCKS_PRIME as u128, expected);
    }
}
