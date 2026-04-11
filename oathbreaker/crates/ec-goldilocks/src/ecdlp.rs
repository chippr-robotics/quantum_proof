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
        // Pollard's rho with Floyd's cycle detection.
    //
    // Given G (generator) and Q = [k]G (target), find k.
    //
    // We maintain a walk point R = [a]G + [b]Q and apply a pseudo-random
    // iteration function based on a partition of the group into 3 sets
    // (determined by x-coordinate mod 3):
    //   S_0: R ← R + Q,  b ← b + 1
    //   S_1: R ← 2R,     a ← 2a, b ← 2b
    //   S_2: R ← R + G,  a ← a + 1
    //
    // Floyd's tortoise-and-hare finds a collision:
    //   [a1]G + [b1]Q = [a2]G + [b2]Q
    // => (a1 - a2)G = (b2 - b1)Q = (b2 - b1)[k]G
    // => k = (a1 - a2) * (b2 - b1)^(-1) mod n

    let n = _curve.order;

    // Guard against degenerate curves with zero order (e.g., placeholder test fixtures).
    if n == 0 {
        return None;
    }

    // Partition function: which set does point R belong to?
    let partition = |r: &AffinePoint| -> u64 {
        match r {
            AffinePoint::Infinity => 0,
            AffinePoint::Finite { x, .. } => x.to_canonical() % 3,
        }
    };

    // Iteration function: advance (R, a, b) by one step.
    let step = |r: &AffinePoint, a: u64, b: u64| -> (AffinePoint, u64, u64) {
        match partition(r) {
            0 => {
                // S_0: R ← R + Q, b ← b + 1
                let new_r = point_add(r, _target, _curve);
                (new_r, a, (b + 1) % n)
            }
            1 => {
                // S_1: R ← 2R, a ← 2a, b ← 2b
                let new_r = point_add(r, r, _curve);
                (new_r, (2 * a) % n, (2 * b) % n)
            }
            _ => {
                // S_2: R ← R + G, a ← a + 1
                let new_r = point_add(r, _generator, _curve);
                (new_r, (a + 1) % n, b)
            }
        }
    };

    // Initialize: tortoise and hare both start at G (a=1, b=0)
    let mut tortoise = (*_generator, 1u64, 0u64);
    let mut hare = (*_generator, 1u64, 0u64);

    // Floyd's cycle detection with a bounded iteration count.
    // For a group of order n, the expected cycle length is O(sqrt(n)).
    // We cap at 4*sqrt(n) + 1000 iterations to avoid infinite loops on
    // degenerate inputs or unlucky partitions.
    let max_iter: u64 = 4 * (n as f64).sqrt().ceil() as u64 + 1000;
    let mut iter_count: u64 = 0;
    loop {
        if iter_count >= max_iter {
            return None; // Exceeded iteration limit; caller should retry with a different seed.
        }
        iter_count += 1;

        // Tortoise takes 1 step
        tortoise = step(&tortoise.0, tortoise.1, tortoise.2);

        // Hare takes 2 steps
        hare = step(&hare.0, hare.1, hare.2);
        hare = step(&hare.0, hare.1, hare.2);

        if tortoise.0 == hare.0 {
            break;
        }
    }

    // Collision found: [a1]G + [b1]Q = [a2]G + [b2]Q
    let (_, a1, b1) = tortoise;
    let (_, a2, b2) = hare;

    // k = (a1 - a2) * (b2 - b1)^(-1) mod n
    let da = if a1 >= a2 { a1 - a2 } else { n - (a2 - a1) };
    let db = if b2 >= b1 { b2 - b1 } else { n - (b1 - b2) };

    if db == 0 {
        return None; // degenerate collision, retry would be needed
    }

    // Compute modular inverse of db mod n using extended GCD
    let db_inv = mod_inverse(db, n)?;
    let k = ((da as u128 * db_inv as u128) % n as u128) as u64;

    // Verify: [k]G should equal Q
    let check = scalar_mul(k, _generator, _curve);
    if check == *_target {
        Some(k)
    } else {
        // The collision may have given k + n/gcd or similar.
        // Try k + n/2 if n is even (unlikely for prime order).
        None
    }
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

/// Compute the modular inverse of a mod m using the extended Euclidean algorithm.
/// Returns None if gcd(a, m) != 1.
fn mod_inverse(a: u64, m: u64) -> Option<u64> {
    if m == 0 {
        return None;
    }
    let (mut old_r, mut r) = (a as i128, m as i128);
    let (mut old_s, mut s) = (1i128, 0i128);

    while r != 0 {
        let quotient = old_r / r;
        let temp_r = r;
        r = old_r - quotient * r;
        old_r = temp_r;
        let temp_s = s;
        s = old_s - quotient * s;
        old_s = temp_s;
    }

    if old_r != 1 {
        return None; // gcd(a, m) != 1, no inverse exists
    }

    // Ensure result is positive
    let result = ((old_s % m as i128) + m as i128) % m as i128;
    Some(result as u64)
}
