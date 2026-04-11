use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible modular multiplier for GF(p).
///
/// Decomposes multiplication into controlled additions (schoolbook method):
/// |a⟩|b⟩|0⟩ → |a⟩|b⟩|a*b mod p⟩
///
/// Gate count: O(n²) Toffoli for n-bit operands.
/// Ancilla: accumulator register (n+1 bits) + reduction workspace.
pub struct ReversibleMultiplier {
    /// Number of bits per operand.
    pub n: usize,
}

impl ReversibleMultiplier {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular multiplication.
    ///
    /// Register layout:
    /// - a[0..n]: first operand (preserved)
    /// - b[0..n]: second operand (preserved)
    /// - result[0..n]: output register (starts at 0, ends with a*b mod p)
    /// - workspace: ancilla qubits for intermediate values
    ///
    /// The multiplication uses schoolbook decomposition:
    /// a * b = Σ_i a_i * b * 2^i
    /// Each bit a_i controls a conditional addition of (b << i) to the accumulator.
    ///
    /// After accumulation, reduce modulo p using the special form:
    /// p = 2^64 - 2^32 + 1, so 2^64 ≡ 2^32 - 1 (mod p).
    ///
    /// Intermediate values are uncomputed via Bennett's compute-copy-uncompute strategy.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible schoolbook multiplication using controlled additions.
        //
        // |a⟩|b⟩|0⟩ → |a⟩|b⟩|a*b mod p⟩
        //
        // Uses Bennett's compute-copy-uncompute pattern:
        // 1. Forward: accumulate partial products via controlled additions
        // 2. Copy: CNOT result to output register
        // 3. Reverse: uncompute accumulator by running forward gates in reverse
        //
        // Each bit a[i] controls the addition of (b << i) to the accumulator.
        // The accumulator is 2n bits wide to hold intermediate products before
        // reduction. After all partial products, reduce mod p.

        let n = self.n;
        let mut gates = Vec::new();

        // Workspace layout:
        //   workspace[0..2n]:   acc   — 2n-bit accumulator for unreduced product
        //   workspace[2n]:      carry — 1-bit carry ancilla for the Cuccaro adder
        //   workspace[2n+1..3n+1]: pp — n-bit partial-product row scratch
        //
        // Total: 3n+1 qubits.
        let acc_offset = workspace_offset;
        let carry_bit = workspace_offset + 2 * n;
        let pp_offset = workspace_offset + 2 * n + 1;

        counter.allocate_ancilla(3 * n + 1);

        // --- Forward pass: schoolbook partial-product accumulation ---
        //
        // For each bit i of `a` (the "multiplier"), conditionally add b << i to acc.
        //
        // Step A: Load partial-product row pp[j] = a[i] AND b[j]  (n Toffoli gates).
        // Step B: Integer-add pp into acc at position i using the Cuccaro ripple-carry
        //         adder.  The add is performed on min(n, 2n - i) bits, accommodating
        //         the remaining columns in acc without overflowing the 2n-bit range.
        // Step C: Unload pp (same Toffoli gates are self-inverse).
        //
        // This gives the correct integer product in acc[0..2n] after all n rows.
        let mut forward_gates_list: Vec<Gate> = Vec::new();

        for i in 0..n {
            // Step A: pp[j] ← a[i] AND b[j]
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: a_offset + i,
                    control2: b_offset + j,
                    target: pp_offset + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }

            // Step B: acc[i..i+n] += pp  via Cuccaro adder (integer addition with carries).
            //   We add `n` bits of pp into acc starting at column i.
            //   The adder needs acc[i+n] as an overflow bit; this is within acc[0..2n]
            //   as long as i + n < 2n  (i.e. i < n), which always holds here.
            let add_width = n; // add all n bits of pp into acc[i..i+n]
            let adder = CuccaroAdder::new(add_width);
            let add_gates = adder.forward_gates(
                pp_offset,       // 'a' input: partial-product row (preserved by adder)
                acc_offset + i,  // 'b' input / output: accumulator at column i
                carry_bit,       // ancilla carry (starts and ends at 0)
                counter,
            );
            forward_gates_list.extend(add_gates);

            // Step C: pp[j] ← 0  (uncompute; Toffoli is self-inverse)
            for j in 0..n {
                let g = Gate::Toffoli {
                    control1: a_offset + i,
                    control2: b_offset + j,
                    target: pp_offset + j,
                };
                counter.record_gate(&g);
                forward_gates_list.push(g);
            }
        }

        // --- Goldilocks modular reduction ---
        //
        // p = 2^64 − 2^32 + 1, so 2^64 ≡ 2^32 − 1  (mod p).
        // For each high bit at position n+k  (k = 0 … n−1):
        //   acc[n+k] represents the value 2^(n+k) = 2^n · 2^k ≡ (2^32 − 1) · 2^k
        //     = 2^(k+32) − 2^k  (mod p).
        // So acc[n+k] = 1 contributes:  +2^(k+32) to the low register (if k+32 < n)
        //                               −2^k       to the low register.
        //
        // We fold by: for each k, if acc[n+k] is set,
        //   • add 2^(k+32) to acc[0..n] (if k+32 < n)  — controlled increment at k+32
        //   • subtract 2^k from acc[0..n]               — controlled decrement at k
        //
        // In the reversible Clifford+T model a "conditional add 1 at position p"
        // requires carry propagation; we represent it here with Toffoli-based
        // carry-ripple through the low register, which is correct for resource counting.
        //
        // For each high bit position h = n + k:
        let mut reduce_gates: Vec<Gate> = Vec::new();
        for k in 0..n {
            let h = acc_offset + n + k; // high bit position

            // Contribution +2^(k+32) mod p — add to low register at bit k+32
            // (only valid when k+32 < n, i.e. k < 32 for 64-bit)
            if k + 32 < n {
                // Step 1: Flip bit k+32 (the +2^(k+32) increment), controlled on h.
                // This is the base CNOT that adds 2^(k+32) when there is no carry.
                let g_flip = Gate::Cnot { control: h, target: acc_offset + k + 32 };
                counter.record_gate(&g_flip);
                reduce_gates.push(g_flip);

                // Step 2: Carry propagation for higher bits (when acc[k+32] was already 1).
                // Conditional increment of acc[k+33..n] when carry propagates from bit k+32.
                let carry_len = n - k - 32 - 1; // number of bits above k+32 within low half
                for carry_step in 0..carry_len {
                    let pos = acc_offset + k + 32 + carry_step;
                    // Compute carry: carry_bit ^= h AND pos
                    let g_carry = Gate::Toffoli {
                        control1: h,
                        control2: pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g_carry);
                    reduce_gates.push(g_carry);

                    // Propagate carry to pos+1
                    let g_sum = Gate::Cnot { control: carry_bit, target: pos + 1 };
                    counter.record_gate(&g_sum);
                    reduce_gates.push(g_sum);

                    // Uncompute carry_bit
                    let g_uncarry = Gate::Toffoli {
                        control1: h,
                        control2: pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g_uncarry);
                    reduce_gates.push(g_uncarry);
                }
            }

            // Contribution −2^k: subtract from acc[k..n].
            // In two's-complement, subtracting 2^k = adding NOT(2^k) + 1.
            // For the resource model we use the controlled-decrement (flip bit k,
            // then borrow-propagate if acc[k] was 0).
            {
                let pos_k = acc_offset + k;
                let g_flip = Gate::Cnot { control: h, target: pos_k };
                counter.record_gate(&g_flip);
                reduce_gates.push(g_flip);

                // Borrow propagation: if original acc[k] was 0, borrow from acc[k+1..].
                // Controlled on h AND NOT acc[k]_after_flip (which equals original acc[k]).
                // We approximate with a Toffoli chain for carry/borrow propagation.
                for borrow_step in 0..(n - k - 1) {
                    let pos = acc_offset + k + borrow_step;
                    let g_borrow = Gate::Toffoli {
                        control1: h,
                        control2: pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g_borrow);
                    reduce_gates.push(g_borrow);

                    let g_prop = Gate::Cnot {
                        control: carry_bit,
                        target: pos + 1,
                    };
                    counter.record_gate(&g_prop);
                    reduce_gates.push(g_prop);

                    let g_unb = Gate::Toffoli {
                        control1: h,
                        control2: pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g_unb);
                    reduce_gates.push(g_unb);
                }
            }
        }
        forward_gates_list.extend(reduce_gates);

        gates.extend(forward_gates_list.clone());

        // --- Copy result to output register ---
        // CNOT the low n bits of the accumulator to the result register.
        for i in 0..n {
            let g = Gate::Cnot {
                control: acc_offset + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // --- Uncompute: reverse the forward gates ---
        // Bennett compute-copy-uncompute: run forward_gates in reverse to restore
        // the accumulator (and pp scratch) to |0⟩.
        for gate in forward_gates_list.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(3 * n + 1);
        gates
    }
}

/// Reversible modular squaring, specialized for better gate count.
///
/// Since both inputs are the same, some gate optimizations are possible.
pub struct ReversibleSquarer {
    pub n: usize,
}

impl ReversibleSquarer {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible squaring: delegates to the multiplier with both inputs
        // pointing to the same register.
        //
        // |a⟩|0⟩ → |a⟩|a² mod p⟩
        //
        // Future optimization: exploit symmetry of cross-terms to reduce
        // gate count by ~25% (each a[i]*a[j] term appears twice, so only
        // one Toffoli + doubling is needed instead of two Toffoli gates).
        let mul = ReversibleMultiplier::new(self.n);
        mul.forward_gates(input_offset, input_offset, result_offset, workspace_offset, counter)
    }
}
