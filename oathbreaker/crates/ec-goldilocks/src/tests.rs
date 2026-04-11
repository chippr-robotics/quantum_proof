#[cfg(test)]
mod tests {
    use crate::curve::{AffinePoint, CurveParams};
    use crate::ecdlp;
    use crate::point_ops::{point_add, point_double, scalar_mul};
    use goldilocks_field::GoldilocksField;

    /// Create a small test curve for unit testing.
    /// Uses a known-good curve over a small prime for fast tests.
    /// The actual Goldilocks curve parameters will come from the Sage scripts.
    fn test_curve() -> CurveParams {
        // Placeholder: will be replaced with real Sage-generated parameters
        // For now, tests are structural — they verify the API works
        todo!("Load test curve parameters from Sage-generated fixture")
    }

    #[test]
    fn test_infinity_identity() {
        // point_add(P, O) = P for any point P
        let p = AffinePoint::Finite {
            x: GoldilocksField::new(1),
            y: GoldilocksField::new(2),
        };
        let inf = AffinePoint::Infinity;

        // We can at least verify the identity property without a real curve
        let curve = CurveParams {
            a: GoldilocksField::ZERO,
            b: GoldilocksField::ZERO,
            order: 0,
            generator: AffinePoint::Infinity,
            field_bits: 64,
        };

        assert_eq!(point_add(&p, &inf, &curve), p);
        assert_eq!(point_add(&inf, &p, &curve), p);
        assert_eq!(point_add(&inf, &inf, &curve), inf);
    }

    #[test]
    fn test_scalar_mul_zero() {
        let p = AffinePoint::Finite {
            x: GoldilocksField::new(1),
            y: GoldilocksField::new(2),
        };
        let curve = CurveParams {
            a: GoldilocksField::ZERO,
            b: GoldilocksField::ZERO,
            order: 0,
            generator: AffinePoint::Infinity,
            field_bits: 64,
        };

        assert_eq!(scalar_mul(0, &p, &curve), AffinePoint::Infinity);
    }

    #[test]
    fn test_point_negation() {
        let p = AffinePoint::Finite {
            x: GoldilocksField::new(5),
            y: GoldilocksField::new(10),
        };
        let neg_p = p.neg();

        match neg_p {
            AffinePoint::Finite { x, y } => {
                assert_eq!(x, GoldilocksField::new(5));
                assert_eq!(y, -GoldilocksField::new(10));
            }
            _ => panic!("Expected finite point"),
        }
    }

    #[test]
    fn test_infinity_negation() {
        assert_eq!(AffinePoint::Infinity.neg(), AffinePoint::Infinity);
    }
}
