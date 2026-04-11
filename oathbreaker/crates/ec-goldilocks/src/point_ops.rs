use crate::curve::{AffinePoint, CurveParams};
use goldilocks_field::GoldilocksField;

/// Add two affine points on the curve. Handles all cases:
/// - P + O = P
/// - O + Q = Q
/// - P + (-P) = O
/// - P + P = 2P (point doubling)
/// - P + Q (general case)
pub fn point_add(p: &AffinePoint, q: &AffinePoint, curve: &CurveParams) -> AffinePoint {
    match (p, q) {
        (AffinePoint::Infinity, _) => *q,
        (_, AffinePoint::Infinity) => *p,
        (
            AffinePoint::Finite { x: x1, y: y1 },
            AffinePoint::Finite { x: x2, y: y2 },
        ) => {
            if x1 == x2 {
                if y1 == y2 {
                    // P == Q, use doubling
                    return point_double(p, curve);
                } else {
                    // P == -Q
                    return AffinePoint::Infinity;
                }
            }

            // General case: lambda = (y2 - y1) / (x2 - x1)
            let dy = *y2 - *y1;
            let dx = *x2 - *x1;
            let dx_inv = dx.inverse().expect("dx should be non-zero here");
            let lambda = dy * dx_inv;

            let x3 = lambda * lambda - *x1 - *x2;
            let y3 = lambda * (*x1 - x3) - *y1;

            AffinePoint::Finite { x: x3, y: y3 }
        }
    }
}

/// Double an affine point on the curve.
/// Uses the tangent slope: lambda = (3x^2 + a) / (2y)
pub fn point_double(p: &AffinePoint, curve: &CurveParams) -> AffinePoint {
    match p {
        AffinePoint::Infinity => AffinePoint::Infinity,
        AffinePoint::Finite { x, y } => {
            if y.to_canonical() == 0 {
                return AffinePoint::Infinity;
            }

            let three = GoldilocksField::from_canonical(3);
            let two = GoldilocksField::from_canonical(2);

            let numerator = three * *x * *x + curve.a;
            let denominator = two * *y;
            let denom_inv = denominator.inverse().expect("2y should be non-zero");
            let lambda = numerator * denom_inv;

            let x3 = lambda * lambda - *x - *x;
            let y3 = lambda * (*x - x3) - *y;

            AffinePoint::Finite { x: x3, y: y3 }
        }
    }
}

/// Scalar multiplication via double-and-add: compute [k]P.
pub fn scalar_mul(k: u64, p: &AffinePoint, curve: &CurveParams) -> AffinePoint {
    if k == 0 {
        return AffinePoint::Infinity;
    }

    let mut result = AffinePoint::Infinity;
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

/// Scalar multiplication using Jacobian coordinates for performance.
pub fn scalar_mul_jacobian(k: u64, p: &AffinePoint, curve: &CurveParams) -> AffinePoint {
    if k == 0 {
        return AffinePoint::Infinity;
    }

    // For now, delegate to affine scalar_mul.
    // TODO: Implement full Jacobian double-and-add for better performance.
    scalar_mul(k, p, curve)
}
