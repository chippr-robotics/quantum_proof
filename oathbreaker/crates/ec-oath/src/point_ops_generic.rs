//! Prime-generic classical EC arithmetic.
//!
//! The legacy [`crate::point_ops`] module operates over [`oath_field::GoldilocksField`]
//! exclusively, which silently corrupts every sub-Goldilocks tier in the
//! Oathbreaker scale (Oath-4/8/16/32). This module provides a parallel
//! implementation that reduces against the curve's actual prime via
//! [`oath_field::PrimeField`].
//!
//! The intent is to use this as the *classical reference oracle* against which
//! the reversible circuit's gate sequence is validated -- once
//! [`reversible-arithmetic`] grows a prime-generic modular reduction (Phase 3
//! of the honest-implementation roadmap), the Oath-4 attack circuit can be
//! checked end-to-end by classically simulating its gate sequence and
//! comparing the index-register output to the corresponding `point_add`
//! result here.
//!
//! Storage of `(x, y)` is still raw `u64`s in `[0, p)`; only the arithmetic
//! differs from the Goldilocks fast path. Callers convert from
//! [`crate::AffinePoint`] (which holds `GoldilocksField` values) by reading
//! `to_canonical()` and feeding the resulting `u64` into
//! [`oath_field::PrimeField::elem`].

use crate::CurveParams;
use oath_field::{PrimeField, PrimeFieldElement};

/// Prime-generic affine point.
///
/// Equivalent to [`crate::AffinePoint`] but the coordinates are stored as
/// [`PrimeFieldElement`]s reduced against the curve's actual modulus rather
/// than the Goldilocks prime. Construct via [`from_affine`] or
/// [`PointP::finite`]. Use [`PointP::to_affine`] to convert back when feeding
/// the result into call sites that still expect the Goldilocks-typed
/// [`crate::AffinePoint`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointP {
    Infinity,
    Finite { x: PrimeFieldElement, y: PrimeFieldElement },
}

impl PointP {
    /// Construct a finite point. Caller is responsible for ensuring the field
    /// matches the surrounding curve.
    pub fn finite(x: PrimeFieldElement, y: PrimeFieldElement) -> Self {
        Self::Finite { x, y }
    }

    /// Construct from a Goldilocks-typed [`crate::AffinePoint`] under the
    /// given prime field. Reduces each coordinate against the new modulus.
    pub fn from_affine(p: &crate::AffinePoint, fp: &PrimeField) -> Self {
        match p {
            crate::AffinePoint::Infinity => Self::Infinity,
            crate::AffinePoint::Finite { x, y } => Self::Finite {
                x: fp.elem(x.to_canonical()),
                y: fp.elem(y.to_canonical()),
            },
        }
    }

    /// Convert back to a Goldilocks-typed [`crate::AffinePoint`]. Coordinates
    /// in `[0, p)` are stored verbatim into `GoldilocksField::from_canonical`,
    /// preserving the small numeric value.
    pub fn to_affine(&self) -> crate::AffinePoint {
        match self {
            Self::Infinity => crate::AffinePoint::Infinity,
            Self::Finite { x, y } => crate::AffinePoint::Finite {
                x: oath_field::GoldilocksField::from_canonical(x.to_canonical()),
                y: oath_field::GoldilocksField::from_canonical(y.to_canonical()),
            },
        }
    }

    /// Negate this point (reflect across the x-axis).
    pub fn neg(&self, fp: &PrimeField) -> Self {
        match *self {
            Self::Infinity => Self::Infinity,
            Self::Finite { x, y } => Self::Finite {
                x,
                y: fp.neg(y),
            },
        }
    }
}

/// Affine point addition over the curve's actual prime modulus.
///
/// Handles the standard cases:
/// - `O + Q = Q`, `P + O = P`
/// - `P + (-P) = O`
/// - `P + P = 2P` (doubling)
/// - General `P + Q`
pub fn point_add(p: &PointP, q: &PointP, curve: &CurveParams) -> PointP {
    let fp = curve.prime_field();
    match (p, q) {
        (PointP::Infinity, _) => *q,
        (_, PointP::Infinity) => *p,
        (
            PointP::Finite { x: x1, y: y1 },
            PointP::Finite { x: x2, y: y2 },
        ) => {
            if x1 == x2 {
                if y1 == y2 {
                    return point_double(p, curve);
                } else {
                    // P == -Q
                    return PointP::Infinity;
                }
            }
            let dy = fp.sub(*y2, *y1);
            let dx = fp.sub(*x2, *x1);
            let dx_inv = fp
                .inverse(dx)
                .expect("dx must be non-zero in this branch");
            let lambda = fp.mul(dy, dx_inv);
            let lambda_sq = fp.square(lambda);
            let x3 = fp.sub(fp.sub(lambda_sq, *x1), *x2);
            let y3 = fp.sub(fp.mul(lambda, fp.sub(*x1, x3)), *y1);
            PointP::Finite { x: x3, y: y3 }
        }
    }
}

/// Affine point doubling over the curve's actual prime modulus.
pub fn point_double(p: &PointP, curve: &CurveParams) -> PointP {
    let fp = curve.prime_field();
    match *p {
        PointP::Infinity => PointP::Infinity,
        PointP::Finite { x, y } => {
            if y.to_canonical() == 0 {
                return PointP::Infinity;
            }
            let three = fp.elem(3);
            let two = fp.elem(2);
            let a = fp.elem(curve.a.to_canonical());
            let numerator = fp.add(fp.mul(fp.mul(three, x), x), a);
            let denominator = fp.mul(two, y);
            let denom_inv = fp
                .inverse(denominator)
                .expect("2y must be non-zero (y == 0 already handled)");
            let lambda = fp.mul(numerator, denom_inv);
            let lambda_sq = fp.square(lambda);
            let x3 = fp.sub(fp.sub(lambda_sq, x), x);
            let y3 = fp.sub(fp.mul(lambda, fp.sub(x, x3)), y);
            PointP::Finite { x: x3, y: y3 }
        }
    }
}

/// Scalar multiplication via double-and-add over the curve's actual prime.
pub fn scalar_mul(k: u64, p: &PointP, curve: &CurveParams) -> PointP {
    if k == 0 {
        return PointP::Infinity;
    }
    let mut result = PointP::Infinity;
    let mut base = *p;
    let mut scalar = k;
    while scalar > 0 {
        if scalar & 1 == 1 {
            result = point_add(&result, &base, curve);
        }
        base = point_double(&base, curve);
        scalar >>= 1;
    }
    result
}

/// Convenience: scalar-multiply the curve's generator.
pub fn scalar_mul_generator(k: u64, curve: &CurveParams) -> PointP {
    let fp = curve.prime_field();
    let g = PointP::from_affine(&curve.generator, &fp);
    scalar_mul(k, &g, curve)
}

#[cfg(test)]
mod tests {
    //! These tests pin the generic primitives against the Oath-4 reference
    //! values from the Python POC at `oathbreaker/qiskit/poc/oath4.py`. Oath-4
    //! is the smallest curve in the framework where the GoldilocksField path
    //! gives wrong answers and the generic path gives right ones, so it is
    //! the natural validation curve.

    use super::*;
    use crate::{AffinePoint, CurveParams};
    use oath_field::GoldilocksField;

    /// Oath-4 test curve as defined in the framework: y^2 = x^3 + x + 6 over
    /// GF(11), generator (2, 4), order 13.
    fn oath4_curve() -> CurveParams {
        CurveParams {
            a: GoldilocksField::new(1),
            b: GoldilocksField::new(6),
            order: 13,
            generator: AffinePoint::Finite {
                x: GoldilocksField::new(2),
                y: GoldilocksField::new(4),
            },
            field_bits: 4,
            prime_modulus: 11,
        }
    }

    /// All thirteen multiples [0]G..[12]G of the Oath-4 generator (2, 4),
    /// computed by hand from y^2 = x^3 + x + 6 mod 11. Each pair satisfies
    /// the curve equation; pairs at index k and 13-k are negatives of each
    /// other (y components sum to 0 mod 11).
    fn oath4_expected_multiples() -> [(Option<u64>, Option<u64>); 13] {
        [
            (None, None),       // [0]G = O
            (Some(2), Some(4)), // [1]G
            (Some(5), Some(9)), // [2]G
            (Some(8), Some(8)), // [3]G
            (Some(10), Some(9)),// [4]G
            (Some(3), Some(5)), // [5]G
            (Some(7), Some(2)), // [6]G
            (Some(7), Some(9)), // [7]G  = -[6]G
            (Some(3), Some(6)), // [8]G  = -[5]G
            (Some(10), Some(2)),// [9]G  = -[4]G
            (Some(8), Some(3)), // [10]G = -[3]G
            (Some(5), Some(2)), // [11]G = -[2]G
            (Some(2), Some(7)), // [12]G = -[1]G
        ]
    }

    fn point_to_pair(p: &PointP) -> (Option<u64>, Option<u64>) {
        match p {
            PointP::Infinity => (None, None),
            PointP::Finite { x, y } => (Some(x.to_canonical()), Some(y.to_canonical())),
        }
    }

    #[test]
    fn generator_is_on_curve_over_real_prime() {
        let curve = oath4_curve();
        let fp = curve.prime_field();
        let g = PointP::from_affine(&curve.generator, &fp);
        if let PointP::Finite { x, y } = g {
            // y^2 ?= x^3 + x + 6  (mod 11)
            let lhs = fp.square(y);
            let rhs = fp.add(
                fp.add(fp.mul(fp.square(x), x), x),
                fp.elem(6),
            );
            assert_eq!(lhs, rhs, "generator (2,4) must satisfy curve eq mod 11");
        } else {
            panic!("generator should be finite");
        }
    }

    #[test]
    fn scalar_mul_recovers_oath4_reference_table() {
        let curve = oath4_curve();
        let expected = oath4_expected_multiples();
        for k in 0..13u64 {
            let got = scalar_mul_generator(k, &curve);
            assert_eq!(
                point_to_pair(&got),
                expected[k as usize],
                "[{}]G mismatch on Oath-4",
                k
            );
        }
    }

    #[test]
    fn scalar_mul_n_returns_infinity() {
        let curve = oath4_curve();
        let inf = scalar_mul_generator(curve.order, &curve);
        assert_eq!(inf, PointP::Infinity, "[n]G must be the point at infinity");
    }

    #[test]
    fn point_add_satisfies_associativity_on_oath4() {
        let curve = oath4_curve();
        for a in 0..13u64 {
            for b in 0..13u64 {
                for c in 0..13u64 {
                    let g_a = scalar_mul_generator(a, &curve);
                    let g_b = scalar_mul_generator(b, &curve);
                    let g_c = scalar_mul_generator(c, &curve);

                    // (g_a + g_b) + g_c
                    let lhs = point_add(&point_add(&g_a, &g_b, &curve), &g_c, &curve);
                    // g_a + (g_b + g_c)
                    let rhs = point_add(&g_a, &point_add(&g_b, &g_c, &curve), &curve);
                    assert_eq!(lhs, rhs, "associativity at ({}, {}, {})", a, b, c);
                }
            }
        }
    }

    #[test]
    fn doubling_matches_self_addition_on_oath4() {
        let curve = oath4_curve();
        for k in 1..13u64 {
            let p = scalar_mul_generator(k, &curve);
            let doubled = point_double(&p, &curve);
            let self_added = point_add(&p, &p, &curve);
            assert_eq!(doubled, self_added, "double vs self-add at k={}", k);
        }
    }

    #[test]
    fn negation_yields_additive_inverse_on_oath4() {
        let curve = oath4_curve();
        let fp = curve.prime_field();
        for k in 1..13u64 {
            let p = scalar_mul_generator(k, &curve);
            let neg = p.neg(&fp);
            let zero = point_add(&p, &neg, &curve);
            assert_eq!(zero, PointP::Infinity, "P + (-P) at k={}", k);
        }
    }

    /// The Goldilocks-only path produces the WRONG answer on Oath-4 because
    /// the field reduction is mod 2^64 - 2^32 + 1 instead of mod 11. This
    /// test exists to make that documented gap concrete: it asserts the
    /// legacy path's first wrong answer.
    #[test]
    fn legacy_goldilocks_path_is_wrong_on_oath4() {
        let curve = oath4_curve();
        // Compute [3]G via the Goldilocks-typed primitives.
        let legacy = crate::point_ops::scalar_mul(3, &curve.generator, &curve);
        let generic = scalar_mul_generator(3, &curve).to_affine();
        assert_ne!(
            legacy, generic,
            "legacy path should disagree with generic path on Oath-4 -- if this \
             test fails, either the legacy path was fixed (good!) or the generic \
             path regressed (bad)."
        );
    }
}
