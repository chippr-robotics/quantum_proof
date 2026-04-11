#[cfg(test)]
mod tests {
    use crate::curve::{AffinePoint, CurveParams};
    use crate::point_ops::{point_add, scalar_mul};
    use goldilocks_field::GoldilocksField;

    /// Create a test curve over the Goldilocks field for unit testing.
    ///
    /// Curve: y² = x³ + x + 3 (a=1, b=3) over GF(p), p = 2^64 - 2^32 + 1.
    /// The generator is found by trying small x values until y² is a
    /// quadratic residue, then computing the square root via Tonelli-Shanks.
    ///
    /// Note: the curve order is a placeholder (= p). Full parameter validation
    /// and true order computation requires SageMath. These parameters are
    /// sufficient for structural and API testing.
    fn test_curve() -> CurveParams {
        let a = GoldilocksField::new(1);
        let b = GoldilocksField::new(3);

        // Try x values until we find one where y² = x³ + ax + b is a QR.
        // x = 0: y² = 3
        let gx = GoldilocksField::new(0);
        let gy_sq = gx * gx * gx + a * gx + b; // y² = 3

        // Use Tonelli-Shanks to compute sqrt(3) mod p.
        let gy = gy_sq.sqrt().expect("y² = 3 should be a quadratic residue mod p");
        debug_assert_eq!(gy * gy, gy_sq, "sqrt verification failed");

        let generator = AffinePoint::Finite { x: gx, y: gy };
        debug_assert!(generator.is_on_curve(&CurveParams {
            a, b, order: 0, generator: AffinePoint::Infinity, field_bits: 64,
        }));

        // Order placeholder — real value from Sage's E.order().
        // By Hasse's theorem, order ≈ p ± O(sqrt(p)).
        let order = GoldilocksField::P;

        CurveParams {
            a,
            b,
            order,
            generator,
            field_bits: 64,
        }
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
