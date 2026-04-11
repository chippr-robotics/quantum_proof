use crate::constants::GOLDILOCKS_PRIME;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

/// An element of GF(p) where p = 2^64 - 2^32 + 1 (the Goldilocks prime).
///
/// All arithmetic is performed modulo p. The internal representation is a
/// canonical u64 value in [0, p).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GoldilocksField(u64);

impl GoldilocksField {
    pub const P: u64 = GOLDILOCKS_PRIME;

    /// The additive identity.
    pub const ZERO: Self = Self(0);

    /// The multiplicative identity.
    pub const ONE: Self = Self(1);

    /// Create a new field element, reducing modulo p.
    pub fn new(value: u64) -> Self {
        Self(Self::reduce(value as u128))
    }

    /// Create a field element from a raw value that is already in [0, p).
    /// # Safety
    /// The caller must ensure `value < p`.
    pub const fn from_canonical(value: u64) -> Self {
        Self(value)
    }

    /// Return the canonical u64 representation in [0, p).
    pub const fn to_canonical(&self) -> u64 {
        self.0
    }

    /// Reduce a u128 value modulo p, exploiting p's special form.
    ///
    /// For p = 2^64 - 2^32 + 1, we have:
    ///   2^64 ≡ 2^32 - 1 (mod p)
    ///
    /// So for x = x_lo + 2^64 * x_hi:
    ///   x mod p = x_lo + x_hi * (2^32 - 1) (mod p)
    ///
    /// A single fold may still leave bits above 64 bits, so we keep folding
    /// until the value is small enough for a final conditional subtraction.
    fn reduce(x: u128) -> u64 {
        const EPSILON: u128 = (1u128 << 32) - 1;

        let x_lo = x as u64;
        let x_hi = (x >> 64) as u64;

        // First fold: x_lo + x_hi * (2^32 - 1).
        let folded = x_lo as u128 + (x_hi as u128) * EPSILON;

        // Second fold: enough to reduce the first fold from < 2^97 to < 2^65.
        let folded_lo = folded as u64;
        let folded_hi = folded >> 64;
        let folded_again = folded_lo as u128 + folded_hi * EPSILON;

        // Final fold: handles the remaining top bit when folded_again >= 2^64.
        let result_lo = folded_again as u64;
        let result_hi = folded_again >> 64;
        let result = result_lo as u128 + result_hi * EPSILON;

        let result = result as u64;
        if result >= Self::P {
            result - Self::P
        } else {
            result
        }
    }

    /// Compute the multiplicative inverse via Fermat's little theorem: a^(p-2) mod p.
    ///
    /// Returns None if self is zero.
    pub fn inverse(&self) -> Option<Self> {
        if self.0 == 0 {
            return None;
        }
        Some(self.pow(crate::constants::P_MINUS_TWO))
    }

    /// Compute self^exp mod p via square-and-multiply.
    pub fn pow(&self, mut exp: u64) -> Self {
        let mut base = *self;
        let mut result = Self::ONE;

        while exp > 0 {
            if exp & 1 == 1 {
                result = result * base;
            }
            base = base * base;
            exp >>= 1;
        }

        result
    }

    /// Compute the Legendre symbol (self / p).
    /// Returns 1 if self is a quadratic residue, -1 if not, 0 if self is zero.
    pub fn legendre(&self) -> i8 {
        if self.0 == 0 {
            return 0;
        }
        let result = self.pow(crate::constants::P_MINUS_ONE_HALF);
        if result.0 == 1 {
            1
        } else {
            -1
        }
    }

    /// Compute the square root of self modulo p, if it exists.
    ///
    /// Uses the Tonelli-Shanks algorithm.
    /// Returns None if self is not a quadratic residue.
    pub fn sqrt(&self) -> Option<Self> {
        if self.0 == 0 {
            return Some(Self::ZERO);
        }
        if self.legendre() != 1 {
            return None;
        }
        // Tonelli-Shanks for p = 2^64 - 2^32 + 1.
        // p - 1 = 2^32 * (2^32 - 1), so S = 32, Q = 2^32 - 1.
        let s: u32 = 32;
        let q: u64 = (1u64 << 32) - 1; // 2^32 - 1 = 0xFFFFFFFF

        // Find a quadratic non-residue. 7 is the multiplicative generator.
        let z = Self::from_canonical(crate::constants::MULTIPLICATIVE_GENERATOR);
        debug_assert_eq!(z.legendre(), -1, "z must be a quadratic non-residue");

        let mut m = s;
        let mut c = z.pow(q);                // z^Q
        let mut t = self.pow(q);             // n^Q
        let mut r = self.pow((q + 1) / 2);   // n^((Q+1)/2)

        loop {
            if t.0 == 0 {
                return Some(Self::ZERO);
            }
            if t.0 == 1 {
                // Return the canonical (smaller) root
                let neg_r = -r;
                if r.0 <= neg_r.0 {
                    return Some(r);
                } else {
                    return Some(neg_r);
                }
            }

            // Find the least i such that t^(2^i) = 1
            let mut i = 0u32;
            let mut tmp = t;
            while tmp.0 != 1 {
                tmp = tmp * tmp;
                i += 1;
                if i >= m {
                    return None; // should not happen if legendre == 1
                }
            }

            // Update: b = c^(2^(m-i-1)), r = r*b, t = t*b^2, c = b^2
            let exp = 1u64 << (m - i - 1);
            let b = c.pow(exp);
            r = r * b;
            let b2 = b * b;
            t = t * b2;
            c = b2;
            m = i;
        }
    }
}

impl Add for GoldilocksField {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        let sum = self.0 as u128 + rhs.0 as u128;
        Self(Self::reduce(sum))
    }
}

impl Sub for GoldilocksField {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        if self.0 >= rhs.0 {
            Self(self.0 - rhs.0)
        } else {
            // self.0 - rhs.0 + p
            Self(Self::P - (rhs.0 - self.0))
        }
    }
}

impl Mul for GoldilocksField {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        let product = self.0 as u128 * rhs.0 as u128;
        Self(Self::reduce(product))
    }
}

impl Neg for GoldilocksField {
    type Output = Self;

    fn neg(self) -> Self {
        if self.0 == 0 {
            Self::ZERO
        } else {
            Self(Self::P - self.0)
        }
    }
}

impl fmt::Debug for GoldilocksField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "GF({})", self.0)
    }
}

impl fmt::Display for GoldilocksField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for GoldilocksField {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}
