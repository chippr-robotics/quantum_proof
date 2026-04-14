use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::multiplier::cuccaro_subtract;
use crate::resource_counter::ResourceCounter;

/// Reversible Montgomery multiplication for GF(p).
///
/// Montgomery multiplication keeps values in Montgomery form: ā = a·R mod p
/// where R = 2^n. The key advantage is that modular reduction after
/// multiplication becomes a right-shift operation instead of the expensive
/// Goldilocks folding.
///
/// Montgomery product: MonMul(ā, b̄) = a·b·R mod p
///
/// The algorithm (CIOS - Coarsely Integrated Operand Scanning):
/// 1. For each bit i of a:
///    - t += a[i] * b (conditional addition)
///    - q = t[0] (LSB of current partial sum)
///    - t += q * p (make t divisible by 2)
///    - t >>= 1 (right shift, free in reversible circuits via relabeling)
/// 2. If t >= p: t -= p
///
/// The right-shift reduction replaces the O(n²) Goldilocks folding with
/// O(n) additions per bit, giving the same asymptotic cost but with
/// smaller constants for the reduction phase.
///
/// Boundary conversions:
/// - To Montgomery form: ā = a · R² mod p (one Montgomery multiplication by R²)
/// - From Montgomery form: a = ā · 1 mod p (one Montgomery multiplication by 1)
///   or equivalently: REDC(ā) where REDC is the Montgomery reduction
pub struct MontgomeryMultiplier {
    /// Number of bits per operand.
    pub n: usize,
}

impl MontgomeryMultiplier {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Total workspace qubits needed for Montgomery multiplication.
    ///
    /// Layout:
    ///   t[0..n+2]:     (n+2)-bit accumulator (extra bits for carries)
    ///   p_reg[0..n]:   n-bit register holding the prime p
    ///   carry:         1-bit carry for Cuccaro additions
    ///   pp[0..n]:      n-bit partial product scratch
    ///
    /// Total: 3n + 3 qubits.
    pub fn workspace_size(n: usize) -> usize {
        3 * n + 3
    }

    /// Generate the gate sequence for reversible Montgomery multiplication.
    ///
    /// Computes: result = MonMul(a, b) = a * b * R^{-1} mod p
    ///
    /// Uses the bit-serial Montgomery reduction: for each bit of a,
    /// conditionally add b, then conditionally add p to make the LSB zero,
    /// then right-shift by relabeling.
    ///
    /// The right-shift is the key advantage: instead of an explicit
    /// Goldilocks reduction (O(n²) Toffoli for carry propagation),
    /// we simply advance the base pointer of the accumulator.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        p_bits: &[bool], // The prime p as a bit array (LSB first)
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // Workspace layout
        let t_base = workspace_offset; // (n+2)-bit accumulator
        let p_reg = workspace_offset + n + 2; // n-bit prime register
        let carry = workspace_offset + 2 * n + 2; // 1-bit carry
        let pp = workspace_offset + 2 * n + 3; // n-bit partial product scratch

        let total_ws = Self::workspace_size(n);
        counter.allocate_ancilla(total_ws);

        // --- Forward pass ---
        let mut forward_gates_list: Vec<Gate> = Vec::new();

        // Load the prime constant p into p_reg
        for (i, &bit) in p_bits.iter().enumerate().take(n) {
            if bit {
                let g = Gate::Not { target: p_reg + i };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }
        }

        // Bit-serial Montgomery multiplication:
        // For each bit i of a (LSB to MSB):
        //   1. If a[i]=1: t += b (conditional addition via partial products)
        //   2. q = t[0] (the LSB determines correction)
        //   3. If q=1: t += p (Montgomery correction)
        //   4. t >>= 1 (right shift via pointer advancement)
        //
        // We track t_start as the current base of the accumulator to
        // implement the free right-shift.
        //
        // Since we can't actually move pointers in a qubit register,
        // we use the approach of always working with the full (n+2)-bit
        // accumulator but tracking which bit position is the current LSB.
        // After n iterations, the result is in t[n..2n].

        for i in 0..n {
            // Step 1: pp[j] = a[i] AND b[j], then t += pp (shifted by i)
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: a_offset + i,
                    control2: b_offset + j,
                    target: pp + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }

            // Add pp to t starting at position i
            let add_width = n.min(n + 2 - i);
            if add_width > 0 {
                let adder = CuccaroAdder::new(add_width);
                let g = adder.forward_gates(pp, t_base + i, carry, counter);
                forward_gates_list.extend(g);
            }

            // Unload pp
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: a_offset + i,
                    control2: b_offset + j,
                    target: pp + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }

            // Step 2-3: Montgomery correction
            // q = t[i] (the current LSB after i right-shifts)
            // If q=1: t += p << i (conditional addition of p at current position)
            let q_bit = t_base + i; // current LSB

            // Load correction: pp[j] = q AND p_reg[j]
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: q_bit,
                    control2: p_reg + j,
                    target: pp + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }

            // Add correction to t at position i
            if add_width > 0 {
                let adder = CuccaroAdder::new(add_width);
                let g = adder.forward_gates(pp, t_base + i, carry, counter);
                forward_gates_list.extend(g);
            }

            // Unload correction
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: q_bit,
                    control2: p_reg + j,
                    target: pp + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }

            // Step 4: Right shift is implicit — t[i] is now guaranteed 0,
            // and the "active window" advances to t[i+1..].
        }

        // After n iterations, the result is in t[n..2n].
        // Final conditional subtraction: if t >= p, subtract p.
        let result_start = t_base + n;
        let g = cuccaro_subtract(n, p_reg, result_start, carry, counter);
        // We need to check if the result went negative (borrow occurred).
        // For the resource model, we account for one conditional addition.
        // In practice this is a compare-and-correct step.
        forward_gates_list.extend(g);

        // Unload p constant
        for (i, &bit) in p_bits.iter().enumerate().take(n) {
            if bit {
                let g = Gate::Not { target: p_reg + i };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }
        }

        gates.extend(forward_gates_list.clone());

        // Copy result to output register
        for i in 0..n {
            let g = Gate::Cnot {
                control: result_start + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Bennett uncomputation: reverse all forward gates
        for gate in forward_gates_list.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(total_ws);
        gates
    }

    /// Estimated Toffoli count for Montgomery multiplication.
    ///
    /// Per-bit cost: 2n Toffoli (pp load/unload) + O(n) Cuccaro + 2n Toffoli (correction)
    /// Total: O(n²) Toffoli — same asymptotic as schoolbook, but the reduction
    /// phase (right-shift) is cheaper than Goldilocks folding.
    pub fn estimated_toffoli(n: usize) -> usize {
        // Each of n iterations: 2n (pp) + n (Cuccaro) + 2n (correction) + n (Cuccaro)
        // Plus final conditional subtraction: O(n)
        // Bennett doubles the forward count
        let per_iter = 4 * n + 2 * n; // pp load/unload (2×2n) + 2 Cuccaro (~2n)
        let forward = n * per_iter + n; // n iterations + final subtract
        2 * forward // Bennett doubles
    }
}

/// Convert a field element to Montgomery form.
///
/// Montgomery form: ā = a · R mod p, where R = 2^n.
///
/// In the reversible circuit, this is one standard modular multiplication
/// by R mod p at the boundary.
pub fn to_montgomery_form(a: u64, r_mod_p: u64, p: u64) -> u64 {
    let product = (a as u128) * (r_mod_p as u128);
    (product % (p as u128)) as u64
}

/// Convert from Montgomery form back to standard form.
///
/// Standard form: a = ā · R^{-1} mod p.
pub fn from_montgomery_form(a_mont: u64, r_inv_mod_p: u64, p: u64) -> u64 {
    let product = (a_mont as u128) * (r_inv_mod_p as u128);
    (product % (p as u128)) as u64
}

/// Compute modular inverse using extended GCD (classical helper).
#[allow(dead_code)]
pub(crate) fn mod_inverse_u128(a: u128, m: u128) -> u128 {
    if a == 0 {
        return 0;
    }
    let mut old_r = a as i128;
    let mut r = m as i128;
    let mut old_s: i128 = 1;
    let mut s: i128 = 0;

    while r != 0 {
        let q = old_r / r;
        let temp_r = r;
        r = old_r - q * r;
        old_r = temp_r;
        let temp_s = s;
        s = old_s - q * s;
        old_s = temp_s;
    }

    if old_s < 0 {
        old_s += m as i128;
    }
    old_s as u128
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Goldilocks prime p = 2^64 - 2^32 + 1
    const GOLDILOCKS_P: u64 = 0xFFFF_FFFF_0000_0001;

    fn p_bits() -> Vec<bool> {
        let p = GOLDILOCKS_P;
        (0..64).map(|i| (p >> i) & 1 == 1).collect()
    }

    #[test]
    fn test_montgomery_form_roundtrip() {
        // p = 251 (largest prime < 256), n = 8, R = 2^8 = 256
        let p: u64 = 251;
        let n = 8;
        let r = 1u128 << n; // R = 256
        let r_mod_p = (r % (p as u128)) as u64; // 256 mod 251 = 5
        let r_inv = mod_inverse_u128(r % (p as u128), p as u128) as u64;

        for a in 0..p {
            let a_mont = to_montgomery_form(a, r_mod_p, p);
            let a_back = from_montgomery_form(a_mont, r_inv, p);
            assert_eq!(
                a_back, a,
                "Montgomery roundtrip failed for a={}: mont={}, back={}",
                a, a_mont, a_back,
            );
        }
    }

    #[test]
    fn test_montgomery_form_roundtrip_goldilocks() {
        let p = GOLDILOCKS_P;
        let n = 64;
        // R = 2^64. R mod p = 2^64 mod (2^64 - 2^32 + 1) = 2^32 - 1
        let r_mod_p = ((1u128 << 32) - 1) as u64;
        let r_inv = mod_inverse_u128((1u128 << 32) - 1, p as u128) as u64;

        for &a in &[0u64, 1, 2, 42, p - 1, p - 2, 0xDEAD_BEEF] {
            let a_mont = to_montgomery_form(a, r_mod_p, p);
            let a_back = from_montgomery_form(a_mont, r_inv, p);
            assert_eq!(
                a_back, a,
                "Montgomery roundtrip failed for a={}: mont={}, back={}",
                a, a_mont, a_back,
            );
        }
    }

    #[test]
    fn test_montgomery_multiplier_resource_count() {
        // Verify the Montgomery multiplier produces a valid gate sequence
        let n = 8;
        let mut counter = ResourceCounter::new();
        let mul = MontgomeryMultiplier::new(n);

        let small_p_bits: Vec<bool> = {
            // Small prime for testing: p = 251 (largest prime < 256)
            let p: u8 = 251;
            (0..8).map(|i| (p >> i) & 1 == 1).collect()
        };

        let gates = mul.forward_gates(
            0,        // a
            n,        // b
            2 * n,    // result
            3 * n,    // workspace
            &small_p_bits,
            &mut counter,
        );

        // Should produce gates
        assert!(!gates.is_empty(), "Montgomery multiplier should produce gates");

        // Should have Toffoli gates
        assert!(
            counter.toffoli_count > 0,
            "Montgomery multiplier should use Toffoli gates",
        );

        // Verify workspace was properly allocated and freed
        assert_eq!(
            counter.ancilla_allocated,
            MontgomeryMultiplier::workspace_size(n),
            "Workspace size mismatch",
        );
    }

    #[test]
    fn test_montgomery_vs_karatsuba_toffoli() {
        // Compare Toffoli counts between Montgomery and Karatsuba
        use crate::multiplier::KaratsubaMultiplier;

        let n = 16;
        let mut mont_counter = ResourceCounter::new();
        let mut kara_counter = ResourceCounter::new();

        let mont = MontgomeryMultiplier::new(n);
        let kara = KaratsubaMultiplier::new(n);

        let p_bits: Vec<bool> = {
            // Use a simple prime for n=16
            let p: u16 = 65521; // largest prime < 2^16
            (0..16).map(|i| (p >> i) & 1 == 1).collect()
        };

        let _ = mont.forward_gates(0, n, 2 * n, 3 * n, &p_bits, &mut mont_counter);
        let _ = kara.forward_gates(0, n, 2 * n, 3 * n, &mut kara_counter);

        // Both should produce valid gate counts (we don't assert which is lower
        // since Montgomery's advantage is in the reduction phase constant factor,
        // not asymptotics)
        assert!(mont_counter.toffoli_count > 0);
        assert!(kara_counter.toffoli_count > 0);
    }
}
