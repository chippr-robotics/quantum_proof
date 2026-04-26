use oath_field::GoldilocksField;
use serde::{Deserialize, Serialize};

/// Parameters for an elliptic curve E: y^2 = x^3 + ax + b over GF(p).
///
/// Storage of `a`, `b`, and the generator's coordinates reuses the
/// `GoldilocksField` u64 representation regardless of the actual prime --
/// see `prime_modulus` for the prime that should actually be used to reduce
/// arithmetic results. The historical Goldilocks-only path leaves
/// `prime_modulus = GOLDILOCKS_PRIME`; sub-Goldilocks tiers (Oath-4, Oath-8,
/// Oath-16, Oath-32) carry their actual prime here so the new generic
/// classical-arithmetic path in `point_ops_generic` can reduce correctly.
///
/// All numeric values are still stored as canonical `u64`s in
/// `[0, prime_modulus)`; only the *operations* differ between the Goldilocks
/// fast path and the generic path.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurveParams {
    /// Coefficient a in the Weierstrass equation.
    pub a: GoldilocksField,
    /// Coefficient b in the Weierstrass equation.
    pub b: GoldilocksField,
    /// The prime order of the curve group #E(GF(p)).
    pub order: u64,
    /// Generator point of the curve group.
    pub generator: AffinePoint,
    /// Number of bits in the field (always 64 for Goldilocks).
    pub field_bits: usize,
    /// The prime modulus `p` of the underlying field GF(p).
    ///
    /// Defaults to the Goldilocks prime (`2^64 − 2^32 + 1`) for backwards
    /// compatibility with the existing benchmark suite. Sub-Goldilocks tiers
    /// load this from the per-tier JSON (`oath4_params.json`, etc.).
    #[serde(default = "default_prime_modulus")]
    pub prime_modulus: u64,
}

fn default_prime_modulus() -> u64 {
    oath_field::constants::GOLDILOCKS_PRIME
}

impl CurveParams {
    /// Construct a [`oath_field::PrimeField`] context for this curve's actual
    /// modulus -- use this when running the generic classical primitives in
    /// [`crate::point_ops_generic`].
    pub fn prime_field(&self) -> oath_field::PrimeField {
        oath_field::PrimeField::new(self.prime_modulus)
    }
}

impl Default for CurveParams {
    /// Backwards-compatible default: `prime_modulus = GOLDILOCKS_PRIME`,
    /// `field_bits = 64`. Used by the older test sites that constructed
    /// `CurveParams` literals before `prime_modulus` was added.
    fn default() -> Self {
        Self {
            a: GoldilocksField::ZERO,
            b: GoldilocksField::ZERO,
            order: 0,
            generator: AffinePoint::Infinity,
            field_bits: 64,
            prime_modulus: GoldilocksField::P,
        }
    }
}

/// A point on the elliptic curve in affine coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AffinePoint {
    /// The point at infinity (identity element).
    Infinity,
    /// A finite point (x, y).
    Finite {
        x: GoldilocksField,
        y: GoldilocksField,
    },
}

impl AffinePoint {
    /// Create a new finite point.
    pub fn new(x: GoldilocksField, y: GoldilocksField) -> Self {
        Self::Finite { x, y }
    }

    /// Return the point at infinity.
    pub fn infinity() -> Self {
        Self::Infinity
    }

    /// Check if this is the point at infinity.
    pub fn is_infinity(&self) -> bool {
        matches!(self, Self::Infinity)
    }

    /// Negate this point (reflect across the x-axis).
    pub fn neg(&self) -> Self {
        match self {
            Self::Infinity => Self::Infinity,
            Self::Finite { x, y } => Self::Finite { x: *x, y: -*y },
        }
    }

    /// Check if this point lies on the given curve.
    pub fn is_on_curve(&self, curve: &CurveParams) -> bool {
        match self {
            Self::Infinity => true,
            Self::Finite { x, y } => {
                // y^2 = x^3 + ax + b
                let y2 = *y * *y;
                let x3 = *x * *x * *x;
                let ax = curve.a * *x;
                y2 == x3 + ax + curve.b
            }
        }
    }
}

/// A point on the elliptic curve in Jacobian projective coordinates.
/// Represents the affine point (X/Z^2, Y/Z^3).
#[derive(Clone, Copy, Debug)]
pub struct JacobianPoint {
    pub x: GoldilocksField,
    pub y: GoldilocksField,
    pub z: GoldilocksField,
}

impl JacobianPoint {
    /// Convert from affine to Jacobian coordinates.
    pub fn from_affine(p: &AffinePoint) -> Self {
        match p {
            AffinePoint::Infinity => Self {
                x: GoldilocksField::ONE,
                y: GoldilocksField::ONE,
                z: GoldilocksField::ZERO,
            },
            AffinePoint::Finite { x, y } => Self {
                x: *x,
                y: *y,
                z: GoldilocksField::ONE,
            },
        }
    }

    /// Convert from Jacobian back to affine coordinates.
    pub fn to_affine(&self) -> AffinePoint {
        if self.z.to_canonical() == 0 {
            return AffinePoint::Infinity;
        }
        let z_inv = self.z.inverse().unwrap();
        let z_inv2 = z_inv * z_inv;
        let z_inv3 = z_inv2 * z_inv;
        AffinePoint::Finite {
            x: self.x * z_inv2,
            y: self.y * z_inv3,
        }
    }
}

/// A point in Modified Jacobian coordinates (X, Y, Z, aZ⁴).
///
/// Modified Jacobian coordinates cache the value aZ⁴ to avoid recomputing
/// it in each doubling. This reduces the doubling cost from 4S + 4M to
/// 3S + 4M by eliminating the Z₁² → Z₁⁴ → aZ₁⁴ chain.
///
/// The affine point is recovered as (X/Z², Y/Z³), same as standard Jacobian.
/// The fourth coordinate aZ⁴ is maintained consistently across doublings:
///   if 2P = (X₃, Y₃, Z₃, aZ₃⁴), then aZ₃⁴ is computed as part of the
///   doubling formula rather than derived from Z₃.
#[derive(Clone, Copy, Debug)]
pub struct ModifiedJacobianPoint {
    pub x: GoldilocksField,
    pub y: GoldilocksField,
    pub z: GoldilocksField,
    /// Cached value: a · Z⁴ mod p
    pub az4: GoldilocksField,
}

impl ModifiedJacobianPoint {
    /// Convert from affine to modified Jacobian coordinates.
    pub fn from_affine(p: &AffinePoint, curve: &CurveParams) -> Self {
        match p {
            AffinePoint::Infinity => Self {
                x: GoldilocksField::ONE,
                y: GoldilocksField::ONE,
                z: GoldilocksField::ZERO,
                az4: GoldilocksField::ZERO,
            },
            AffinePoint::Finite { x, y } => Self {
                x: *x,
                y: *y,
                z: GoldilocksField::ONE,
                az4: curve.a, // Z=1, so aZ⁴ = a·1 = a
            },
        }
    }

    /// Convert from modified Jacobian back to affine coordinates.
    pub fn to_affine(&self) -> AffinePoint {
        if self.z.to_canonical() == 0 {
            return AffinePoint::Infinity;
        }
        let z_inv = self.z.inverse().unwrap();
        let z_inv2 = z_inv * z_inv;
        let z_inv3 = z_inv2 * z_inv;
        AffinePoint::Finite {
            x: self.x * z_inv2,
            y: self.y * z_inv3,
        }
    }

    /// Convert to standard Jacobian (drop the cached aZ⁴).
    pub fn to_jacobian(&self) -> JacobianPoint {
        JacobianPoint {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}
