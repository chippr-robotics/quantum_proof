//! Generic prime-field arithmetic for arbitrary primes that fit in `u64`.
//!
//! This module exists alongside [`crate::GoldilocksField`] to support curves
//! whose modulus is **not** the Goldilocks prime. The Oathbreaker tier table
//! includes Oath-4 (p = 11), Oath-8 (p = 251), Oath-16, Oath-32 and Oath-64;
//! only Oath-64 sits on the Goldilocks prime. Until this module landed, every
//! sub-Goldilocks tier in the framework was a resource-counting placeholder.
//!
//! All arithmetic uses `u128` intermediates so the implementation is correct
//! for any `2 < p < 2^64`. The implementation is constant-time-naive (uses
//! `%`); cryptographic constant-time is not a goal here -- this is a classical
//! reference for the reversible circuit, never exposed to remote attackers.
//!
//! For Goldilocks specifically, callers can keep using [`crate::GoldilocksField`]
//! to get the special-form fast reduction; an equivalence test in this module
//! pins the generic path against the special-form one for Goldilocks-prime
//! values.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Element of GF(p) for a prime `p` carried at runtime.
///
/// Internally the value is canonical: always in `[0, p)`. Callers obtain
/// elements via the parent [`PrimeField`] context, which carries the modulus
/// and produces consistent elements via [`PrimeField::elem`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrimeFieldElement {
    value: u64,
    /// Modulus carried per-element so binary ops can verify they share a field.
    /// For `p < 2^63` this fits in 64 bits with margin.
    modulus: u64,
}

impl PrimeFieldElement {
    /// Canonical representative in `[0, p)`.
    #[inline]
    pub const fn to_canonical(&self) -> u64 {
        self.value
    }

    /// Modulus this element is reduced against.
    #[inline]
    pub const fn modulus(&self) -> u64 {
        self.modulus
    }

    #[inline]
    fn assert_same_field(&self, other: &Self) {
        debug_assert_eq!(
            self.modulus, other.modulus,
            "binary op across distinct prime fields ({} vs {})",
            self.modulus, other.modulus
        );
    }
}

impl fmt::Debug for PrimeFieldElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fp({} mod {})", self.value, self.modulus)
    }
}

impl fmt::Display for PrimeFieldElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// Runtime-parameterised prime field GF(p) for `2 < p < 2^63`.
///
/// Construct one per curve and use it as a factory for [`PrimeFieldElement`]s.
/// The modulus is stored on the context so all elements produced via the same
/// `PrimeField` share the same `p`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PrimeField {
    modulus: u64,
}

impl PrimeField {
    /// Construct a new field. Panics if `p <= 2`.
    ///
    /// `p == 2` is invalid for elliptic-curve use (the standard short
    /// Weierstrass formulas are not defined in characteristic 2), so we reject
    /// it. Any prime fitting in `u64` (including the Goldilocks prime, which
    /// exceeds `2^63`) is accepted.
    pub const fn new(modulus: u64) -> Self {
        assert!(modulus > 2, "PrimeField requires modulus > 2");
        Self { modulus }
    }

    /// Modulus of this field.
    #[inline]
    pub const fn modulus(&self) -> u64 {
        self.modulus
    }

    /// Construct an element by reducing `value` mod `p`.
    #[inline]
    pub const fn elem(&self, value: u64) -> PrimeFieldElement {
        PrimeFieldElement {
            value: value % self.modulus,
            modulus: self.modulus,
        }
    }

    /// Additive identity in this field.
    #[inline]
    pub const fn zero(&self) -> PrimeFieldElement {
        PrimeFieldElement {
            value: 0,
            modulus: self.modulus,
        }
    }

    /// Multiplicative identity in this field.
    #[inline]
    pub const fn one(&self) -> PrimeFieldElement {
        PrimeFieldElement {
            value: 1,
            modulus: self.modulus,
        }
    }

    /// `a + b` mod `p`.
    pub fn add(&self, a: PrimeFieldElement, b: PrimeFieldElement) -> PrimeFieldElement {
        a.assert_same_field(&b);
        debug_assert_eq!(a.modulus, self.modulus);
        // u128 absorbs the carry for primes near 2^64 (e.g. Goldilocks).
        let sum = a.value as u128 + b.value as u128;
        let m = self.modulus as u128;
        let reduced = if sum >= m { (sum - m) as u64 } else { sum as u64 };
        PrimeFieldElement {
            value: reduced,
            modulus: self.modulus,
        }
    }

    /// `a - b` mod `p`.
    pub fn sub(&self, a: PrimeFieldElement, b: PrimeFieldElement) -> PrimeFieldElement {
        a.assert_same_field(&b);
        debug_assert_eq!(a.modulus, self.modulus);
        let value = if a.value >= b.value {
            a.value - b.value
        } else {
            self.modulus - (b.value - a.value)
        };
        PrimeFieldElement {
            value,
            modulus: self.modulus,
        }
    }

    /// `-a` mod `p`.
    pub fn neg(&self, a: PrimeFieldElement) -> PrimeFieldElement {
        debug_assert_eq!(a.modulus, self.modulus);
        let value = if a.value == 0 {
            0
        } else {
            self.modulus - a.value
        };
        PrimeFieldElement {
            value,
            modulus: self.modulus,
        }
    }

    /// `a * b` mod `p`. Uses u128 intermediate to absorb the cross product.
    pub fn mul(&self, a: PrimeFieldElement, b: PrimeFieldElement) -> PrimeFieldElement {
        a.assert_same_field(&b);
        debug_assert_eq!(a.modulus, self.modulus);
        // p < 2^63 → product < 2^126, fits in u128. The `%` is generic and
        // not constant-time; that is fine for a classical reference oracle.
        let product = (a.value as u128) * (b.value as u128);
        let reduced = (product % (self.modulus as u128)) as u64;
        PrimeFieldElement {
            value: reduced,
            modulus: self.modulus,
        }
    }

    /// `a * a` mod `p`. Same cost as `mul`.
    #[inline]
    pub fn square(&self, a: PrimeFieldElement) -> PrimeFieldElement {
        self.mul(a, a)
    }

    /// `base^exp` mod `p` via square-and-multiply.
    pub fn pow(&self, base: PrimeFieldElement, mut exp: u64) -> PrimeFieldElement {
        debug_assert_eq!(base.modulus, self.modulus);
        let mut result = self.one();
        let mut acc = base;
        while exp > 0 {
            if exp & 1 == 1 {
                result = self.mul(result, acc);
            }
            acc = self.square(acc);
            exp >>= 1;
        }
        result
    }

    /// Multiplicative inverse via Fermat: `a^(p-2)` mod `p`. None if `a == 0`.
    pub fn inverse(&self, a: PrimeFieldElement) -> Option<PrimeFieldElement> {
        debug_assert_eq!(a.modulus, self.modulus);
        if a.value == 0 {
            return None;
        }
        Some(self.pow(a, self.modulus - 2))
    }

    /// Legendre symbol `(a / p)` returning 1, 0, or -1.
    pub fn legendre(&self, a: PrimeFieldElement) -> i8 {
        debug_assert_eq!(a.modulus, self.modulus);
        if a.value == 0 {
            return 0;
        }
        let result = self.pow(a, (self.modulus - 1) / 2);
        if result.value == 1 {
            1
        } else {
            // For odd primes the only non-trivial value of a^((p-1)/2) is p-1.
            -1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GOLDILOCKS_PRIME;
    use crate::GoldilocksField;

    /// For each small prime, exercise the field axioms and the operator
    /// table against a hand-checkable reference.
    fn axioms(p: u64) {
        let f = PrimeField::new(p);
        for x in 0..p.min(64) {
            for y in 0..p.min(64) {
                let a = f.elem(x);
                let b = f.elem(y);

                assert_eq!(f.add(a, b).to_canonical(), (x + y) % p);
                assert_eq!(f.sub(a, b).to_canonical(), (x + p - (y % p)) % p);
                assert_eq!(f.mul(a, b).to_canonical(), (x * y) % p);

                assert_eq!(f.add(a, f.zero()), a, "additive identity");
                assert_eq!(f.mul(a, f.one()), a, "multiplicative identity");
                assert_eq!(f.add(a, f.neg(a)), f.zero(), "additive inverse");

                if x != 0 {
                    let inv = f.inverse(a).expect("nonzero element has inverse");
                    assert_eq!(f.mul(a, inv), f.one(), "multiplicative inverse");
                }
            }
        }
    }

    #[test]
    fn axioms_p_11() {
        axioms(11);
    }

    #[test]
    fn axioms_p_251() {
        axioms(251);
    }

    #[test]
    fn axioms_p_65521() {
        axioms(65521);
    }

    #[test]
    fn equivalence_with_goldilocks_field_for_small_inputs() {
        let f = PrimeField::new(GOLDILOCKS_PRIME);
        for &(x, y) in &[
            (0u64, 0u64),
            (1, 0),
            (0, 1),
            (1, 1),
            (123_456, 789),
            (GOLDILOCKS_PRIME - 1, 1),
            (GOLDILOCKS_PRIME - 1, GOLDILOCKS_PRIME - 1),
            (1u64 << 32, 1u64 << 32),
        ] {
            let a = f.elem(x);
            let b = f.elem(y);
            let ga = GoldilocksField::new(x);
            let gb = GoldilocksField::new(y);

            assert_eq!(
                f.add(a, b).to_canonical(),
                (ga + gb).to_canonical(),
                "add({}, {})",
                x,
                y
            );
            assert_eq!(
                f.sub(a, b).to_canonical(),
                (ga - gb).to_canonical(),
                "sub({}, {})",
                x,
                y
            );
            assert_eq!(
                f.mul(a, b).to_canonical(),
                (ga * gb).to_canonical(),
                "mul({}, {})",
                x,
                y
            );
            assert_eq!(
                f.neg(a).to_canonical(),
                (-ga).to_canonical(),
                "neg({})",
                x
            );
            if x != 0 {
                assert_eq!(
                    f.inverse(a).unwrap().to_canonical(),
                    ga.inverse().unwrap().to_canonical(),
                    "inverse({})",
                    x
                );
            }
        }
    }

    #[test]
    fn pow_matches_small_exponents() {
        let f = PrimeField::new(11);
        let a = f.elem(2);
        // 2^0..2^10 mod 11: 1,2,4,8,5,10,9,7,3,6,1
        let expected = [1u64, 2, 4, 8, 5, 10, 9, 7, 3, 6, 1];
        for (e, want) in expected.iter().enumerate() {
            assert_eq!(f.pow(a, e as u64).to_canonical(), *want, "2^{} mod 11", e);
        }
    }

    #[test]
    fn fermat_inverse_recovers_each_nonzero() {
        let f = PrimeField::new(11);
        for v in 1..11 {
            let a = f.elem(v);
            let inv = f.inverse(a).unwrap();
            assert_eq!(f.mul(a, inv).to_canonical(), 1, "inverse of {} mod 11", v);
        }
    }

    #[test]
    fn legendre_symbol() {
        let f = PrimeField::new(11);
        // QRs mod 11: {1, 3, 4, 5, 9}; non-QRs: {2, 6, 7, 8, 10}.
        let qrs: std::collections::HashSet<u64> = [1u64, 3, 4, 5, 9].into_iter().collect();
        for v in 1..11u64 {
            let a = f.elem(v);
            let want = if qrs.contains(&v) { 1 } else { -1 };
            assert_eq!(f.legendre(a), want, "legendre({}/11)", v);
        }
        assert_eq!(f.legendre(f.zero()), 0);
    }

    #[test]
    #[should_panic]
    fn cross_field_op_panics_in_debug() {
        let f1 = PrimeField::new(11);
        let f2 = PrimeField::new(13);
        let a = f1.elem(3);
        let b = f2.elem(3);
        // Triggers debug_assert; in release the result is undefined-but-bounded.
        let _ = f1.add(a, b);
    }
}
