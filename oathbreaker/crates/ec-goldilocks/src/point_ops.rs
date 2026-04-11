use crate::curve::{AffinePoint, CurveParams, JacobianPoint};
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

/// Jacobian point doubling: 2P in Jacobian coordinates.
///
/// Input:  P = (X1, Y1, Z1) in Jacobian
/// Output: 2P = (X3, Y3, Z3) in Jacobian
///
/// Cost: 4 multiplications + 4 squarings + 6 additions.
/// No inversions.
pub fn jacobian_double(p: &JacobianPoint, curve: &CurveParams) -> JacobianPoint {
    // Identity check: Z == 0 → point at infinity
    if p.z.to_canonical() == 0 {
        return *p;
    }

    let two = GoldilocksField::from_canonical(2);
    let three = GoldilocksField::from_canonical(3);
    let four = GoldilocksField::from_canonical(4);
    let eight = GoldilocksField::from_canonical(8);

    // A = Y1²
    let a = p.y * p.y;
    // B = 4 · X1 · A
    let b = four * p.x * a;
    // C = 8 · A²
    let c = eight * a * a;
    // D = 3 · X1² + a_curve · Z1⁴
    let z2 = p.z * p.z;
    let z4 = z2 * z2;
    let d = three * p.x * p.x + curve.a * z4;

    // X3 = D² - 2·B
    let x3 = d * d - two * b;
    // Y3 = D · (B - X3) - C
    let y3 = d * (b - x3) - c;
    // Z3 = 2 · Y1 · Z1
    let z3 = two * p.y * p.z;

    JacobianPoint { x: x3, y: y3, z: z3 }
}

/// Mixed Jacobian–affine point addition.
///
/// Adds an affine point Q = (x2, y2) to a Jacobian point P = (X1, Y1, Z1).
/// "Mixed" means one input is affine (Z=1), which saves multiplications.
///
/// Cost: 8 multiplications + 3 squarings + 7 additions.
/// No inversions (the key optimization over affine addition).
///
/// This is the critical subroutine for the windowed scalar multiplication:
/// the accumulator is in Jacobian coordinates, and the precomputed table
/// entries are in affine coordinates.
pub fn jacobian_mixed_add(
    p: &JacobianPoint,
    q: &AffinePoint,
    curve: &CurveParams,
) -> JacobianPoint {
    // Handle identity cases
    if p.z.to_canonical() == 0 {
        return JacobianPoint::from_affine(q);
    }
    match q {
        AffinePoint::Infinity => return *p,
        AffinePoint::Finite { x: x2, y: y2 } => {
            // Z1² and Z1³
            let z1_sq = p.z * p.z;
            let z1_cu = z1_sq * p.z;

            // U1 = X1 (already in Jacobian)
            // U2 = x2 · Z1²
            let u2 = *x2 * z1_sq;
            // S1 = Y1 (already in Jacobian)
            // S2 = y2 · Z1³
            let s2 = *y2 * z1_cu;

            // H = U2 - U1
            let h = u2 - p.x;
            // R = S2 - S1
            let r = s2 - p.y;

            if h.to_canonical() == 0 {
                if r.to_canonical() == 0 {
                    // P == Q, use doubling
                    return jacobian_double(p, curve);
                } else {
                    // P == -Q → infinity
                    return JacobianPoint {
                        x: GoldilocksField::ONE,
                        y: GoldilocksField::ONE,
                        z: GoldilocksField::ZERO,
                    };
                }
            }

            let h_sq = h * h;
            let h_cu = h_sq * h;

            // X3 = R² - H³ - 2·U1·H²
            let two = GoldilocksField::from_canonical(2);
            let x3 = r * r - h_cu - two * p.x * h_sq;
            // Y3 = R·(U1·H² - X3) - S1·H³
            let y3 = r * (p.x * h_sq - x3) - p.y * h_cu;
            // Z3 = Z1 · H
            let z3 = p.z * h;

            JacobianPoint { x: x3, y: y3, z: z3 }
        }
    }
}

/// Scalar multiplication using Jacobian coordinates for performance.
///
/// Uses Jacobian doubling (no inversions) and mixed addition (affine table
/// entries added to Jacobian accumulator). A single inversion is performed
/// at the end to convert back to affine.
///
/// This mirrors the structure of the reversible circuit: the accumulator
/// stays in Jacobian throughout, with one final affine conversion.
pub fn scalar_mul_jacobian(k: u64, p: &AffinePoint, curve: &CurveParams) -> AffinePoint {
    if k == 0 {
        return AffinePoint::Infinity;
    }

    // Left-to-right double-and-add in Jacobian coordinates.
    // Find the highest set bit position.
    let bits = 64 - k.leading_zeros();

    let mut result = JacobianPoint::from_affine(&AffinePoint::Infinity);

    for i in (0..bits).rev() {
        // Double the accumulator
        result = jacobian_double(&result, curve);
        // If bit i is set, add the base point (affine)
        if (k >> i) & 1 == 1 {
            result = jacobian_mixed_add(&result, p, curve);
        }
    }

    // Single inversion to convert back to affine
    result.to_affine()
}
