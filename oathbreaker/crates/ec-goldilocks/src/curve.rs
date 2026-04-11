use goldilocks_field::GoldilocksField;
use serde::{Deserialize, Serialize};

/// Parameters for an elliptic curve E: y^2 = x^3 + ax + b over GF(p).
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
