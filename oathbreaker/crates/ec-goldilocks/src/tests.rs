#[cfg(test)]
mod ec_tests {
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
        let gy = gy_sq
            .sqrt()
            .expect("y² = 3 should be a quadratic residue mod p");
        debug_assert_eq!(gy * gy, gy_sq, "sqrt verification failed");

        let generator = AffinePoint::Finite { x: gx, y: gy };
        debug_assert!(generator.is_on_curve(&CurveParams {
            a,
            b,
            order: 0,
            generator: AffinePoint::Infinity,
            field_bits: 64,
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

    #[test]
    fn test_jacobian_scalar_mul_matches_affine() {
        // Verify that Jacobian scalar multiplication produces the same
        // results as affine scalar multiplication for various scalars.
        let curve = test_curve();
        let g = &curve.generator;

        // Test several scalar values
        for k in [1u64, 2, 3, 5, 7, 10, 42, 100, 255, 1000] {
            let affine_result = scalar_mul(k, g, &curve);
            let jacobian_result = crate::point_ops::scalar_mul_jacobian(k, g, &curve);
            assert_eq!(
                affine_result, jacobian_result,
                "Jacobian and affine scalar mul disagree for k={}",
                k
            );
        }
    }

    #[test]
    fn test_jacobian_mixed_add_matches_affine() {
        // Verify Jacobian mixed addition against affine reference.
        use crate::curve::JacobianPoint;
        use crate::point_ops::{jacobian_mixed_add, point_double};

        let curve = test_curve();
        let g = &curve.generator;

        // Compute 2G via affine doubling
        let two_g_affine = point_double(g, &curve);

        // Compute 2G via Jacobian: start with G in Jacobian, add G (affine)
        let g_jac = JacobianPoint::from_affine(g);
        let two_g_jac = jacobian_mixed_add(&g_jac, g, &curve);
        let two_g_from_jac = two_g_jac.to_affine();

        assert_eq!(
            two_g_affine, two_g_from_jac,
            "Jacobian mixed add (G + G) should equal affine doubling (2G)"
        );

        // Compute 3G = 2G + G via both methods
        let three_g_affine = point_add(g, &two_g_affine, &curve);
        let three_g_jac = jacobian_mixed_add(&two_g_jac, g, &curve);
        let three_g_from_jac = three_g_jac.to_affine();

        assert_eq!(
            three_g_affine, three_g_from_jac,
            "Jacobian mixed add (2G + G) should equal affine add"
        );
    }

    #[test]
    fn test_jacobian_double_matches_affine() {
        use crate::curve::JacobianPoint;
        use crate::point_ops::{jacobian_double, point_double};

        let curve = test_curve();
        let g = &curve.generator;

        // Compute 2G via affine
        let two_g_affine = point_double(g, &curve);

        // Compute 2G via Jacobian
        let g_jac = JacobianPoint::from_affine(g);
        let two_g_jac = jacobian_double(&g_jac, &curve);
        let two_g_from_jac = two_g_jac.to_affine();

        assert_eq!(
            two_g_affine, two_g_from_jac,
            "Jacobian doubling should match affine doubling"
        );

        // Compute 4G = 2(2G) via both methods
        let four_g_affine = point_double(&two_g_affine, &curve);
        let four_g_jac = jacobian_double(&two_g_jac, &curve);
        let four_g_from_jac = four_g_jac.to_affine();

        assert_eq!(
            four_g_affine, four_g_from_jac,
            "Jacobian double(2G) should match affine double(2G)"
        );
    }

    #[test]
    fn test_modified_jacobian_double_matches_affine() {
        use crate::curve::ModifiedJacobianPoint;
        use crate::point_ops::{modified_jacobian_double, point_double};

        let curve = test_curve();
        let g = &curve.generator;

        // Compute 2G via affine
        let two_g_affine = point_double(g, &curve);

        // Compute 2G via modified Jacobian
        let g_mj = ModifiedJacobianPoint::from_affine(g, &curve);
        let two_g_mj = modified_jacobian_double(&g_mj, &curve);
        let two_g_from_mj = two_g_mj.to_affine();

        assert_eq!(
            two_g_affine, two_g_from_mj,
            "Modified Jacobian doubling should match affine doubling"
        );

        // Chain: 4G = 2(2G), 8G = 2(4G), etc.
        let mut mj = two_g_mj;
        let mut affine = two_g_affine;
        for k in [4, 8, 16, 32] {
            affine = point_double(&affine, &curve);
            mj = modified_jacobian_double(&mj, &curve);
            let mj_affine = mj.to_affine();
            assert_eq!(
                affine, mj_affine,
                "Modified Jacobian chain doubling disagrees at {}G",
                k,
            );
        }
    }

    #[test]
    fn test_modified_jacobian_scalar_mul() {
        // Test modified Jacobian doubling through a scalar multiplication
        // that uses repeated doubling.
        use crate::curve::ModifiedJacobianPoint;
        use crate::point_ops::{jacobian_mixed_add, modified_jacobian_double};

        let curve = test_curve();
        let g = &curve.generator;

        // Compute [k]G using modified Jacobian doubling + mixed addition
        for k in [2u64, 3, 5, 7, 10, 42, 100, 255] {
            let expected = scalar_mul(k, g, &curve);

            // Left-to-right double-and-add using modified Jacobian
            let bits = 64 - k.leading_zeros();
            let mut result_mj = ModifiedJacobianPoint::from_affine(&AffinePoint::Infinity, &curve);

            for i in (0..bits).rev() {
                // Double in modified Jacobian
                result_mj = modified_jacobian_double(&result_mj, &curve);
                if (k >> i) & 1 == 1 {
                    // Add G (affine) — convert to standard Jacobian for mixed add
                    let jac = result_mj.to_jacobian();
                    let sum = jacobian_mixed_add(&jac, g, &curve);
                    // Convert back to modified Jacobian
                    let z2 = sum.z * sum.z;
                    let z4 = z2 * z2;
                    result_mj = ModifiedJacobianPoint {
                        x: sum.x,
                        y: sum.y,
                        z: sum.z,
                        az4: curve.a * z4,
                    };
                }
            }

            let actual = result_mj.to_affine();
            assert_eq!(
                expected, actual,
                "Modified Jacobian scalar mul disagrees for k={}",
                k,
            );
        }
    }

    #[test]
    fn test_on_curve_verification() {
        let curve = test_curve();
        let g = &curve.generator;

        // Generator should be on the curve
        assert!(g.is_on_curve(&curve), "Generator should be on curve");

        // Scalar multiples should also be on curve
        for k in [2u64, 3, 5, 10] {
            let p = scalar_mul(k, g, &curve);
            assert!(p.is_on_curve(&curve), "[{}]G should be on curve", k);
        }
    }
}
