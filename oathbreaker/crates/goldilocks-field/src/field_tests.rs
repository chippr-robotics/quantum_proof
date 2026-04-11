#[cfg(test)]
mod tests {
    use crate::field::GoldilocksField;

    #[test]
    fn test_addition_identity() {
        let a = GoldilocksField::new(42);
        assert_eq!(a + GoldilocksField::ZERO, a);
    }

    #[test]
    fn test_multiplication_identity() {
        let a = GoldilocksField::new(42);
        assert_eq!(a * GoldilocksField::ONE, a);
    }

    #[test]
    fn test_additive_inverse() {
        let a = GoldilocksField::new(42);
        assert_eq!(a + (-a), GoldilocksField::ZERO);
    }

    #[test]
    fn test_multiplicative_inverse() {
        let a = GoldilocksField::new(42);
        let a_inv = a.inverse().unwrap();
        assert_eq!(a * a_inv, GoldilocksField::ONE);
    }

    #[test]
    fn test_zero_has_no_inverse() {
        assert!(GoldilocksField::ZERO.inverse().is_none());
    }

    #[test]
    fn test_subtraction() {
        let a = GoldilocksField::new(10);
        let b = GoldilocksField::new(3);
        assert_eq!((a - b).to_canonical(), 7);
    }

    #[test]
    fn test_subtraction_underflow() {
        let a = GoldilocksField::new(3);
        let b = GoldilocksField::new(10);
        // a - b should wrap around mod p
        let result = a - b;
        assert_eq!(result + b, a);
    }

    #[test]
    fn test_edge_cases() {
        let zero = GoldilocksField::ZERO;
        let one = GoldilocksField::ONE;
        let p_minus_1 = GoldilocksField::from_canonical(GoldilocksField::P - 1);

        // p - 1 + 1 = 0 (mod p)
        assert_eq!(p_minus_1 + one, zero);

        // (p-1) * (p-1) = 1 (mod p), since p-1 ≡ -1
        assert_eq!(p_minus_1 * p_minus_1, one);
    }

    #[test]
    fn test_pow() {
        let a = GoldilocksField::new(3);
        // 3^0 = 1
        assert_eq!(a.pow(0), GoldilocksField::ONE);
        // 3^1 = 3
        assert_eq!(a.pow(1), a);
        // 3^2 = 9
        assert_eq!(a.pow(2).to_canonical(), 9);
    }

    #[test]
    fn test_commutativity() {
        let a = GoldilocksField::new(123456789);
        let b = GoldilocksField::new(987654321);

        assert_eq!(a + b, b + a);
        assert_eq!(a * b, b * a);
    }

    #[test]
    fn test_distributivity() {
        let a = GoldilocksField::new(111);
        let b = GoldilocksField::new(222);
        let c = GoldilocksField::new(333);

        assert_eq!(a * (b + c), a * b + a * c);
    }

    #[test]
    fn test_legendre_symbol() {
        let one = GoldilocksField::ONE;
        assert_eq!(one.legendre(), 1); // 1 is always a QR
        assert_eq!(GoldilocksField::ZERO.legendre(), 0);
    }
}

#[cfg(test)]
mod proptests {
    use crate::field::GoldilocksField;
    use proptest::prelude::*;

    prop_compose! {
        fn arb_field_element()(value in 0u64..GoldilocksField::P) -> GoldilocksField {
            GoldilocksField::from_canonical(value)
        }
    }

    proptest! {
        #[test]
        fn prop_add_commutative(a in arb_field_element(), b in arb_field_element()) {
            prop_assert_eq!(a + b, b + a);
        }

        #[test]
        fn prop_mul_commutative(a in arb_field_element(), b in arb_field_element()) {
            prop_assert_eq!(a * b, b * a);
        }

        #[test]
        fn prop_add_identity(a in arb_field_element()) {
            prop_assert_eq!(a + GoldilocksField::ZERO, a);
        }

        #[test]
        fn prop_mul_identity(a in arb_field_element()) {
            prop_assert_eq!(a * GoldilocksField::ONE, a);
        }

        #[test]
        fn prop_additive_inverse(a in arb_field_element()) {
            prop_assert_eq!(a + (-a), GoldilocksField::ZERO);
        }

        #[test]
        fn prop_multiplicative_inverse(
            a in arb_field_element().prop_filter("non-zero", |a| a.to_canonical() != 0)
        ) {
            let inv = a.inverse().unwrap();
            prop_assert_eq!(a * inv, GoldilocksField::ONE);
        }

        #[test]
        fn prop_distributive(
            a in arb_field_element(),
            b in arb_field_element(),
            c in arb_field_element()
        ) {
            prop_assert_eq!(a * (b + c), a * b + a * c);
        }
    }
}
