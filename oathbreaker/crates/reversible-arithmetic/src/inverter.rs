use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::multiplier::cuccaro_subtract;
use crate::resource_counter::ResourceCounter;

/// Reversible modular inversion via Fermat's little theorem.
///
/// Computes a^(p-2) mod p via reversible square-and-multiply.
/// For p = 2^64 - 2^32 + 1:
///   p - 2 = 0xFFFF_FFFE_FFFF_FFFF
///   Hamming weight ≈ 63, so ~63 multiplications + 63 squarings.
///
/// Each intermediate squaring must be uncomputed to free ancilla qubits.
///
/// Gate cost: O(n^2.585) Toffoli with Karatsuba multiplier (125 mul/sq operations).
pub struct FermatInverter {
    /// Number of bits in the field.
    pub n: usize,
}

impl FermatInverter {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular inversion via Fermat.
    ///
    /// This is the most expensive single operation in the Shor circuit.
    /// Each step requires a reversible multiplication plus uncomputation.
    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        let p_minus_2: u64 = 0xFFFF_FFFE_FFFF_FFFF;

        let acc_reg = workspace_offset;
        let sq_work = workspace_offset + n;
        let mul_work = workspace_offset + 2 * n;

        counter.allocate_ancilla(4 * n + 1);

        let g = Gate::Not { target: acc_reg };
        counter.record_gate(&g);
        gates.push(g);

        for bit_pos in (0..64).rev() {
            let bit_set = (p_minus_2 >> bit_pos) & 1 == 1;

            let squarer = crate::multiplier::KaratsubaSquarer::new(n);
            let sq_gates = squarer.forward_gates(acc_reg, sq_work, mul_work, counter);
            gates.extend(sq_gates);

            for i in 0..n {
                let g1 = Gate::Cnot {
                    control: sq_work + i,
                    target: acc_reg + i,
                };
                let g2 = Gate::Cnot {
                    control: acc_reg + i,
                    target: sq_work + i,
                };
                let g3 = Gate::Cnot {
                    control: sq_work + i,
                    target: acc_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }

            for i in 0..n {
                let g = Gate::Cnot {
                    control: acc_reg + i,
                    target: sq_work + i,
                };
                counter.record_gate(&g);
                gates.push(g);
            }

            if bit_set {
                let mul = crate::multiplier::KaratsubaMultiplier::new(n);
                let mul_gates = mul.forward_gates(
                    acc_reg,
                    input_offset,
                    sq_work,
                    mul_work,
                    counter,
                );
                gates.extend(mul_gates);

                for i in 0..n {
                    let g1 = Gate::Cnot {
                        control: sq_work + i,
                        target: acc_reg + i,
                    };
                    let g2 = Gate::Cnot {
                        control: acc_reg + i,
                        target: sq_work + i,
                    };
                    let g3 = Gate::Cnot {
                        control: sq_work + i,
                        target: acc_reg + i,
                    };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }

                for i in 0..n {
                    let g = Gate::Cnot {
                        control: acc_reg + i,
                        target: sq_work + i,
                    };
                    counter.record_gate(&g);
                    gates.push(g);
                }
            }
        }

        for i in 0..n {
            let g = Gate::Cnot {
                control: acc_reg + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Workspace left dirty (accumulator still holds result).
        // Callers must wrap in Bennett uncomputation for clean ancilla.

        gates
    }
}

/// Reversible modular inversion via binary GCD (Kaliski / Roetteler method).
///
/// Computes a⁻¹ mod p using the extended binary GCD algorithm in exactly
/// 2n iterations.  Each iteration uses conditional swaps and arithmetic
/// controlled by parity and comparison bits, avoiding data-dependent branching.
///
/// Gate cost: O(n²) Toffoli — asymptotically better than Fermat's O(n^2.585)
/// because it uses O(n) additions/subtractions per iteration (each O(n) Toffoli)
/// over O(n) iterations, with no full multiplications.
///
/// Reference: Roetteler et al., "Quantum Resource Estimates for Computing
/// Elliptic Curve Discrete Logarithms", ASIACRYPT 2017, Section 4.
///
/// # Algorithm overview
///
/// Phase 1 (2n iterations): extended binary GCD
///   Maintains state (u, v, r, s) with u initialized to p, v to a.
///   Each iteration:
///   1. Conditionally swap (u,v) and (r,s) so u is the register to halve
///   2. If both u,v are odd: u -= v, r += s
///   3. Right-shift u, left-shift s
///   4. Reverse conditional swap
///   After 2n iterations: gcd(p,a) = u = 1, r = a⁻¹ · 2^k mod p.
///
/// Phase 2 (2n halvings): Montgomery correction
///   Multiply r by 2⁻²ⁿ mod p via repeated halving:
///   if r is even: r /= 2; if odd: r = (r + p) / 2.
///
/// Result: r = a⁻¹ mod p.
pub struct BinaryGcdInverter {
    pub n: usize,
}

impl BinaryGcdInverter {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Total workspace qubits for the Binary GCD inverter.
    pub fn workspace_size(n: usize) -> usize {
        // u(n) + v(n) + r(n) + s(n) + temp(n) + swap_cond(1) + both_odd(1) + carry(1)
        5 * n + 3
    }

    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // --- Workspace layout ---
        let u_reg = workspace_offset;
        let v_reg = u_reg + n;
        let r_reg = v_reg + n;
        let s_reg = r_reg + n;
        let temp = s_reg + n; // n-bit scratch for conditional add/sub
        let swap_cond = temp + n;
        let both_odd = swap_cond + 1;
        let carry = both_odd + 1;

        let ws_size = Self::workspace_size(n);
        counter.allocate_ancilla(ws_size);

        // === Forward computation ===
        let mut forward_gates: Vec<Gate> = Vec::new();

        // --- Initialize registers ---
        // u = p (Goldilocks prime: 2^n - 2^(n/2) + 1)
        // For the resource model we load p bit-by-bit via NOT gates.
        // Goldilocks form: bits 0 and (n/2)..n-1 are set, bits 1..(n/2-1) are clear.
        let half = n / 2;
        // Bit 0 = 1
        let g = Gate::Not { target: u_reg };
        counter.record_gate(&g);
        forward_gates.push(g);
        // Bits half..n-1 = 1
        for i in half..n {
            let g = Gate::Not { target: u_reg + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // v = input (copy via CNOT)
        for i in 0..n {
            let g = Gate::Cnot {
                control: input_offset + i,
                target: v_reg + i,
            };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // r = 0 (already zero)
        // s = 1 (set bit 0)
        let g = Gate::Not { target: s_reg };
        counter.record_gate(&g);
        forward_gates.push(g);

        // --- Phase 1: 2n iterations of extended binary GCD ---
        for _iter in 0..(2 * n) {
            // Step 1: Compute swap condition.
            // Swap if: (u is odd AND v is even) → we want the even one in u position
            //      OR: (both odd AND v >= u)    → we want the larger in u position
            //
            // Simplified: swap if v should be halved instead of u.
            // swap_cond = (u[0] AND NOT v[0]) OR (u[0] AND v[0] AND NOT u_gt_v)
            //
            // For the resource model, we compute this as:
            //   swap_cond = u[0] AND (NOT v[0] OR NOT u_gt_v_when_both_odd)
            //
            // We approximate by checking: swap if u is odd and (v is even or v >= u).
            // This is equivalent to: swap if u[0]=1 AND (v[0]=0 OR comparison says v>=u).
            //
            // For gate counting: computing the swap condition costs O(n) Toffoli
            // (for the comparison) plus O(1) logic gates.

            // Compute comparison u > v into a temporary via subtraction.
            // We subtract v from u using Cuccaro, capture the borrow-out, and reverse.
            // The borrow-out = 1 iff u < v (unsigned).
            // We store NOT(borrow) = (u >= v) into swap_cond temporarily.
            //
            // For resource counting: a comparison costs the same as a subtraction = O(n) Toffoli.
            // We model it as a Cuccaro subtract of u from v into temp, check carry, uncompute.

            // Approximate: comparison costs 2n Toffoli (forward Cuccaro + reverse)
            {
                let mut tmp_counter = ResourceCounter::new();
                let adder = CuccaroAdder::new(n);
                let cmp_fwd = adder.forward_gates(u_reg, temp, carry, &mut tmp_counter);
                // Record the forward comparison gates
                for g in &cmp_fwd {
                    counter.record_gate(g);
                    forward_gates.push(g.clone());
                }
                // The carry bit tells us about the comparison
                // swap_cond = u[0] AND (NOT v[0] OR carry_result)
                // For simplicity, set swap_cond based on u[0] and v[0]:
                let g = Gate::Toffoli {
                    control1: u_reg, // u[0] = u is odd
                    control2: v_reg, // v[0] placeholder (we use it for condition logic)
                    target: swap_cond,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
                // Reverse comparison
                for g in cmp_fwd.iter().rev() {
                    let inv = g.inverse();
                    counter.record_gate(&inv);
                    forward_gates.push(inv);
                }
            }

            // Step 2: Conditional swap (u, v) controlled on swap_cond
            // CSWAP(c, a, b) = 3 Toffoli per bit
            for i in 0..n {
                let g1 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: v_reg + i,
                    target: u_reg + i,
                };
                let g2 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: u_reg + i,
                    target: v_reg + i,
                };
                let g3 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: v_reg + i,
                    target: u_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }

            // Conditional swap (r, s) controlled on swap_cond
            for i in 0..n {
                let g1 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: s_reg + i,
                    target: r_reg + i,
                };
                let g2 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: r_reg + i,
                    target: s_reg + i,
                };
                let g3 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: s_reg + i,
                    target: r_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }

            // Step 3: Compute both_odd = u[0] AND v[0]
            let g = Gate::Toffoli {
                control1: u_reg,
                control2: v_reg,
                target: both_odd,
            };
            counter.record_gate(&g);
            forward_gates.push(g);

            // Step 4: If both odd, u -= v (conditional subtraction)
            // Conditional subtract: compute temp[i] = both_odd AND v[i],
            // then u -= temp via Cuccaro reverse, then uncompute temp.
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: both_odd,
                    control2: v_reg + i,
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }
            let sub_gates = cuccaro_subtract(n, temp, u_reg, carry, counter);
            forward_gates.extend(sub_gates);
            // Uncompute temp
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: both_odd,
                    control2: v_reg + i,
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }

            // Step 5: If both odd, r += s (conditional addition)
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: both_odd,
                    control2: s_reg + i,
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }
            let adder = CuccaroAdder::new(n);
            let add_gates = adder.forward_gates(temp, r_reg, carry, counter);
            forward_gates.extend(add_gates);
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: both_odd,
                    control2: s_reg + i,
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }

            // Step 6: Uncompute both_odd
            let g = Gate::Toffoli {
                control1: u_reg,
                control2: v_reg,
                target: both_odd,
            };
            counter.record_gate(&g);
            forward_gates.push(g);

            // Step 7: Right-shift u by 1 (u is now even after swap + possible subtraction)
            // Shift: swap adjacent bits from LSB to MSB.
            // u[0] ← u[1], u[1] ← u[2], ..., u[n-2] ← u[n-1], u[n-1] ← 0
            // Implemented as a chain of CNOT swaps.
            for i in 0..(n - 1) {
                let g1 = Gate::Cnot {
                    control: u_reg + i + 1,
                    target: u_reg + i,
                };
                let g2 = Gate::Cnot {
                    control: u_reg + i,
                    target: u_reg + i + 1,
                };
                let g3 = Gate::Cnot {
                    control: u_reg + i + 1,
                    target: u_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }

            // Step 8: Left-shift s by 1
            // s[n-1] ← s[n-2], ..., s[1] ← s[0], s[0] ← 0
            for i in (1..n).rev() {
                let g1 = Gate::Cnot {
                    control: s_reg + i - 1,
                    target: s_reg + i,
                };
                let g2 = Gate::Cnot {
                    control: s_reg + i,
                    target: s_reg + i - 1,
                };
                let g3 = Gate::Cnot {
                    control: s_reg + i - 1,
                    target: s_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }

            // Step 9: Reverse conditional swaps (restore original ordering
            // for clean uncomputation via Bennett's pattern).
            // Swap (r, s) back
            for i in 0..n {
                let g1 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: s_reg + i,
                    target: r_reg + i,
                };
                let g2 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: r_reg + i,
                    target: s_reg + i,
                };
                let g3 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: s_reg + i,
                    target: r_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }
            // Swap (u, v) back
            for i in 0..n {
                let g1 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: v_reg + i,
                    target: u_reg + i,
                };
                let g2 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: u_reg + i,
                    target: v_reg + i,
                };
                let g3 = Gate::Toffoli {
                    control1: swap_cond,
                    control2: v_reg + i,
                    target: u_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }

            // Uncompute swap_cond (same gate, self-inverse)
            let g = Gate::Toffoli {
                control1: u_reg,
                control2: v_reg,
                target: swap_cond,
            };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // --- Phase 2: Montgomery correction (2n halvings mod p) ---
        // r currently holds a⁻¹ · 2^k mod p.  We halve 2n times to get a⁻¹ mod p.
        // Halving mod p: if r is even, r/2; if r is odd, (r+p)/2.
        //
        // For each halving:
        //   1. If r[0]=1 (odd): add p to r via conditional addition
        //   2. Right-shift r by 1 (now r is even after the conditional add)
        for _halving in 0..(2 * n) {
            // Conditional add p to r if r is odd (r[0] = 1)
            // Load p into temp controlled on r[0], add temp to r, uncompute temp
            // p has bits: bit 0 = 1, bits half..n-1 = 1

            // temp[0] = r[0] AND 1 = r[0] (bit 0 of p is 1)
            let g = Gate::Cnot {
                control: r_reg,
                target: temp,
            };
            counter.record_gate(&g);
            forward_gates.push(g);

            // temp[i] = r[0] AND p[i] for bits where p[i]=1
            for i in half..n {
                let g = Gate::Cnot {
                    control: r_reg, // r[0] as control (p[i]=1 for these bits)
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }

            // r += temp (add conditional p)
            let adder = CuccaroAdder::new(n);
            let add_gates = adder.forward_gates(temp, r_reg, carry, counter);
            forward_gates.extend(add_gates);

            // Uncompute temp
            for i in half..n {
                let g = Gate::Cnot {
                    control: r_reg,
                    target: temp + i,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }
            let g = Gate::Cnot {
                control: r_reg,
                target: temp,
            };
            counter.record_gate(&g);
            forward_gates.push(g);

            // Right-shift r by 1 (r is now even)
            for i in 0..(n - 1) {
                let g1 = Gate::Cnot {
                    control: r_reg + i + 1,
                    target: r_reg + i,
                };
                let g2 = Gate::Cnot {
                    control: r_reg + i,
                    target: r_reg + i + 1,
                };
                let g3 = Gate::Cnot {
                    control: r_reg + i + 1,
                    target: r_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                forward_gates.push(g1);
                forward_gates.push(g2);
                forward_gates.push(g3);
            }
        }

        // === Apply forward, copy, Bennett reverse ===
        gates.extend(forward_gates.clone());

        // Copy r to result
        for i in 0..n {
            let g = Gate::Cnot {
                control: r_reg + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Bennett uncomputation: reverse all forward gates
        for gate in forward_gates.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(ws_size);
        gates
    }
}
