use crate::curve::{AffinePoint, CurveParams};
use crate::point_ops::{point_add, scalar_mul};
use std::collections::HashMap;

/// Solve the ECDLP: given G and Q = [k]G, find k.
///
/// Uses Pollard's rho algorithm: O(sqrt(n)) time, O(1) space.
/// This is the primary solver for the Goldilocks curve (~2^32 iterations).
pub fn pollard_rho(
    _generator: &AffinePoint,
    _target: &AffinePoint,
    _curve: &CurveParams,
) -> Option<u64> {
    // TODO: Implement Pollard's rho with Floyd's cycle detection.
    //
    // Algorithm outline:
    // 1. Define a partition function S = S_1 ∪ S_2 ∪ S_3 based on x-coordinate
    // 2. Define an iteration function f(R) that moves through the group:
    //    - S_1: R = R + Q (add target)
    //    - S_2: R = 2R (double)
    //    - S_3: R = R + G (add generator)
    // 3. Track coefficients: R = [a]G + [b]Q
    // 4. Use Floyd's tortoise-and-hare to find a collision
    // 5. Collision gives: [a1]G + [b1]Q = [a2]G + [b2]Q
    //    => k = (a1 - a2) * (b2 - b1)^(-1) mod n
    todo!("Pollard's rho ECDLP solver")
}

/// Solve the ECDLP via Baby-step Giant-step (BSGS).
///
/// O(sqrt(n)) time and space. For n ≈ 2^64, requires ~2^32 entries (~32 GB).
/// Feasible but memory-intensive.
pub fn baby_step_giant_step(
    generator: &AffinePoint,
    target: &AffinePoint,
    curve: &CurveParams,
) -> Option<u64> {
    let n = curve.order;
    let m = (n as f64).sqrt().ceil() as u64;

    // Baby step: compute [j]G for j = 0, 1, ..., m-1
    let mut baby_steps: HashMap<AffinePoint, u64> = HashMap::new();
    let mut current = AffinePoint::Infinity;
    for j in 0..m {
        baby_steps.insert(current, j);
        current = point_add(&current, generator, curve);
    }

    // Giant step: compute Q - [i * m]G for i = 0, 1, ..., m-1
    let neg_mg = scalar_mul(n - m, generator, curve); // [-m]G = [n - m]G
    let mut gamma = *target;
    for i in 0..m {
        if let Some(&j) = baby_steps.get(&gamma) {
            let k = (i * m + j) % n;
            return Some(k);
        }
        gamma = point_add(&gamma, &neg_mg, curve);
    }

    None
}

/// Solve the ECDLP via brute force: iterate [1]G, [2]G, ... until match.
///
/// O(n) time. Only feasible for small scalars or testing.
pub fn brute_force(
    generator: &AffinePoint,
    target: &AffinePoint,
    curve: &CurveParams,
    max_iterations: u64,
) -> Option<u64> {
    let mut current = AffinePoint::Infinity;
    for k in 0..max_iterations.min(curve.order) {
        if current == *target {
            return Some(k);
        }
        current = point_add(&current, generator, curve);
    }
    None
}
