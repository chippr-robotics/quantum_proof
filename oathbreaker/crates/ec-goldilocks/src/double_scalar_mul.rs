use crate::curve::{AffinePoint, CurveParams};
use crate::point_ops::{point_add, scalar_mul};

/// Compute the double-scalar multiplication [a]G + [b]Q.
///
/// This is the classical reference implementation for verifying the
/// coherent group-action circuit. The circuit must produce identical
/// results on all basis-state inputs.
///
/// In Shor's ECDLP algorithm, this is the group homomorphism:
///   f(a, b) = [a]G + [b]Q
/// where G is the generator and Q = [k]G is the target public key.
pub fn double_scalar_mul(
    a: u64,
    generator: &AffinePoint,
    b: u64,
    target_q: &AffinePoint,
    curve: &CurveParams,
) -> AffinePoint {
    let ag = scalar_mul(a, generator, curve);
    let bq = scalar_mul(b, target_q, curve);
    point_add(&ag, &bq, curve)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::CurveParams;
    use goldilocks_field::GoldilocksField;

    #[test]
    fn test_double_scalar_identity_cases() {
        // Without real curve params, test the algebraic properties
        // [0]G + [0]Q = O
        // [1]G + [0]Q = G
        // [0]G + [1]Q = Q
        // These will be tested with real params once Sage generates them.
    }

    #[test]
    fn test_double_scalar_linearity() {
        // For Q = [k]G: [a]G + [b]Q = [a]G + [bk]G = [a + bk]G
        // This is the key identity that Shor's algorithm exploits.
        // Will be tested with real Oath-64 curve parameters.
    }
}
