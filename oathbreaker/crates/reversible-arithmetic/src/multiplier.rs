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
                pp_offset,      // 'a' input: partial-product row (preserved by adder)
                acc_offset + i, // 'b' input / output: accumulator at column i
                carry_bit,      // ancilla carry (starts and ends at 0)
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
                let g_flip = Gate::Cnot {
                    control: h,
                    target: acc_offset + k + 32,
                };
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
                    let g_sum = Gate::Cnot {
                        control: carry_bit,
                        target: pos + 1,
                    };
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
                let g_flip = Gate::Cnot {
                    control: h,
                    target: pos_k,
                };
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
        mul.forward_gates(
            input_offset,
            input_offset,
            result_offset,
            workspace_offset,
            counter,
        )
    }
}

// ===========================================================================
// Karatsuba multiplication — O(n^1.585) Toffoli gates
// ===========================================================================

/// Compute the workspace size needed by `schoolbook_integer_mul` for n-bit
/// operands.  Layout: pp[0..n] + carry(1) = n+1 qubits.
fn schoolbook_int_ws(n: usize) -> usize {
    n + 1
}

/// Schoolbook integer multiplication (partial-product accumulation only).
///
/// Computes acc[0..2n] = a[0..n] * b[0..n] as an integer (no modular
/// reduction).  The workspace (pp scratch + carry) is returned to |0⟩.
///
/// Workspace layout at `ws`:
///   ws[0..n):  pp   — partial-product row scratch
///   ws[n]:     carry — 1-bit carry for the Cuccaro adder
fn schoolbook_integer_mul(
    n: usize,
    a: usize,
    b: usize,
    acc: usize,
    ws: usize,
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    let pp = ws;
    let carry = ws + n;
    let mut gates = Vec::new();

    for i in 0..n {
        // Load pp[j] = a[i] AND b[j]
        for j in 0..n {
            let g = Gate::Toffoli {
                control1: a + i,
                control2: b + j,
                target: pp + j,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // acc[i..i+n] += pp via Cuccaro adder
        let adder = CuccaroAdder::new(n);
        let add_gates = adder.forward_gates(pp, acc + i, carry, counter);
        gates.extend(add_gates);

        // Unload pp (Toffoli is self-inverse)
        for j in 0..n {
            let g = Gate::Toffoli {
                control1: a + i,
                control2: b + j,
                target: pp + j,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
    }

    gates
}

/// Symmetry-optimized schoolbook integer squaring.
///
/// Computes acc[0..2n] = a[0..n]² by exploiting the symmetry of the partial
/// product matrix: each cross-term a[i]*a[j] (i≠j) appears twice, so we
/// compute it once and place it at position i+j+1 (shifted by 1 = ×2).
/// Diagonal terms a[i]² = a[i] are added via CNOT (no Toffoli).
///
/// Saves ~50% of the partial-product Toffoli compared to schoolbook multiply.
///
/// Workspace: pp[0..n-1] + carry(1) = n qubits (fits in schoolbook_int_ws).
fn schoolbook_integer_square(
    n: usize,
    a: usize,
    acc: usize,
    ws: usize,
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    let pp = ws;
    let carry = ws + n; // carry bit (reused from schoolbook_int_ws allocation)
    let mut gates = Vec::new();

    // --- Phase 1: Cross terms (i < j) → a[i]*a[j] at position i+j+1 ---
    //
    // For each row i, the cross terms are a[i]*a[j] for j = i+1..n-1.
    // These go into acc starting at position 2i+2 (= i+(i+1)+1) with
    // width (n-1-i).
    for i in 0..n {
        let row_width = n - 1 - i;
        if row_width == 0 {
            break;
        }

        // Load pp[k] = a[i] AND a[i+1+k]
        for k in 0..row_width {
            let j = i + 1 + k;
            let g = Gate::Toffoli {
                control1: a + i,
                control2: a + j,
                target: pp + k,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Add pp[0..row_width] to acc at position 2*i+2
        let add_pos = 2 * i + 2;
        let add_width = row_width.min(2 * n - add_pos);
        if add_width > 0 {
            let adder = CuccaroAdder::new(add_width);
            let add_gates = adder.forward_gates(pp, acc + add_pos, carry, counter);
            gates.extend(add_gates);
        }

        // Unload pp (Toffoli is self-inverse)
        for k in 0..row_width {
            let j = i + 1 + k;
            let g = Gate::Toffoli {
                control1: a + i,
                control2: a + j,
                target: pp + k,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
    }

    // --- Phase 2: Diagonal terms → a[i] at position 2i ---
    for i in 0..n {
        let pos = 2 * i;
        if pos < 2 * n {
            let g = Gate::Cnot {
                control: a + i,
                target: acc + pos,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
    }

    gates
}

/// Goldilocks modular reduction: fold acc[n..2n] into acc[0..n].
///
/// Uses p = 2^n − 2^(n/2) + 1, so 2^n ≡ 2^(n/2) − 1 (mod p).
///
/// `carry` is a 1-bit ancilla that starts and ends at 0.
fn goldilocks_reduce(
    n: usize,
    acc: usize,
    carry: usize,
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    let half = n / 2; // Goldilocks split: n/2 for any field size
    let mut gates = Vec::new();

    for k in 0..n {
        let h = acc + n + k;

        // +2^(k+half) contribution
        if k + half < n {
            let g_flip = Gate::Cnot {
                control: h,
                target: acc + k + half,
            };
            counter.record_gate(&g_flip);
            gates.push(g_flip);

            let carry_len = n - k - half - 1;
            for s in 0..carry_len {
                let pos = acc + k + half + s;
                let g1 = Gate::Toffoli {
                    control1: h,
                    control2: pos,
                    target: carry,
                };
                let g2 = Gate::Cnot {
                    control: carry,
                    target: pos + 1,
                };
                let g3 = Gate::Toffoli {
                    control1: h,
                    control2: pos,
                    target: carry,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }
        }

        // −2^k contribution (borrow propagation)
        {
            let g_flip = Gate::Cnot {
                control: h,
                target: acc + k,
            };
            counter.record_gate(&g_flip);
            gates.push(g_flip);

            for s in 0..(n - k - 1) {
                let pos = acc + k + s;
                let g1 = Gate::Toffoli {
                    control1: h,
                    control2: pos,
                    target: carry,
                };
                let g2 = Gate::Cnot {
                    control: carry,
                    target: pos + 1,
                };
                let g3 = Gate::Toffoli {
                    control1: h,
                    control2: pos,
                    target: carry,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }
        }
    }

    gates
}

/// Cuccaro addition with carry-out capture.
///
/// Computes sum[0..n+1] = sum[0..n] + add[0..n], where sum[n] receives the
/// carry-out.  `add` is preserved.  `carry` is a 1-bit ancilla (starts/ends 0).
///
/// This inlines the Cuccaro MAJ/UMA pattern with a CNOT between the chains
/// to capture the carry-out into sum[n].
fn add_with_carryout(
    n: usize,
    add: usize,   // n-bit register (preserved)
    sum: usize,   // (n+1)-bit register: [0..n] overwritten with sum, [n] gets carry-out
    carry: usize, // 1-bit ancilla
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    let mut gates = Vec::new();

    // MAJ(x, y, z): CNOT(z→y), CNOT(z→x), Toffoli(x,y→z)
    macro_rules! maj {
        ($x:expr, $y:expr, $z:expr) => {{
            let g1 = Gate::Cnot {
                control: $z,
                target: $y,
            };
            let g2 = Gate::Cnot {
                control: $z,
                target: $x,
            };
            let g3 = Gate::Toffoli {
                control1: $x,
                control2: $y,
                target: $z,
            };
            counter.record_gate(&g1);
            counter.record_gate(&g2);
            counter.record_gate(&g3);
            gates.push(g1);
            gates.push(g2);
            gates.push(g3);
        }};
    }

    // UMA(x, y, z): Toffoli(x,y→z), CNOT(z→x), CNOT(x→y)
    macro_rules! uma {
        ($x:expr, $y:expr, $z:expr) => {{
            let g1 = Gate::Toffoli {
                control1: $x,
                control2: $y,
                target: $z,
            };
            let g2 = Gate::Cnot {
                control: $z,
                target: $x,
            };
            let g3 = Gate::Cnot {
                control: $x,
                target: $y,
            };
            counter.record_gate(&g1);
            counter.record_gate(&g2);
            counter.record_gate(&g3);
            gates.push(g1);
            gates.push(g2);
            gates.push(g3);
        }};
    }

    // Forward MAJ chain
    // In Cuccaro: a=add (preserved), b=sum (overwritten), carry_in=carry
    maj!(carry, sum, add);
    for i in 1..n {
        maj!(add + i - 1, sum + i, add + i);
    }

    // Capture carry-out: add[n-1] holds carry after last MAJ
    let g_co = Gate::Cnot {
        control: add + n - 1,
        target: sum + n,
    };
    counter.record_gate(&g_co);
    gates.push(g_co);

    // Reverse UMA chain (restores add, deposits sum)
    for i in (1..n).rev() {
        uma!(add + i - 1, sum + i, add + i);
    }
    uma!(carry, sum, add);

    gates
}

/// Reversible integer subtraction: b -= a (a preserved).
///
/// Computed by running the Cuccaro adder gate sequence in reverse order.
/// Since Cuccaro forward computes b += a, the reverse computes b -= a.
/// All gates are self-inverse, so reversing the order suffices.
///
/// Cost: O(n) Toffoli + O(2n) CNOT (same as addition).
/// `carry` is a 1-bit ancilla that starts and ends at 0.
pub fn cuccaro_subtract(
    n: usize,
    a: usize,     // n-bit register (preserved)
    b: usize,     // n-bit register (overwritten with b - a)
    carry: usize, // 1-bit ancilla
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    // Generate the forward addition gates using a temporary counter
    // to avoid double-counting in the real counter.
    let mut tmp_counter = ResourceCounter::new();
    let adder = CuccaroAdder::new(n);
    let fwd = adder.forward_gates(a, b, carry, &mut tmp_counter);

    // Run in reverse for subtraction; record each gate in the real counter.
    let mut gates = Vec::with_capacity(fwd.len());
    for gate in fwd.iter().rev() {
        let inv = gate.inverse();
        counter.record_gate(&inv);
        gates.push(inv);
    }
    gates
}

/// Compute the workspace needed by the recursive Karatsuba integer multiply.
///
/// Each level allocates its own intermediates plus three separate sub-workspace
/// regions (one per sub-multiplication) so that dirty intermediates from one
/// sub-call don't collide with the next.
pub fn karatsuba_int_ws(n: usize) -> usize {
    if n <= 8 {
        return schoolbook_int_ws(n);
    }
    let h = n / 2;
    let h_hi = n - h;
    let h_sum = h.max(h_hi) + 1; // size of sa/sb registers

    // Level intermediates: z2_reg(2*h_hi) + sa(h_sum) + sb(h_sum) + z1(2*h_sum) + 3 carries
    let level = 2 * h_hi + h_sum + h_sum + 2 * h_sum + 3;

    // Three separate sub-workspaces
    level + karatsuba_int_ws(h) + karatsuba_int_ws(h_hi) + karatsuba_int_ws(h_sum)
}

/// Recursive Karatsuba integer multiplication (or squaring).
///
/// Computes acc[0..2n] = a[0..n] * b[0..n] (integer product, no modular
/// reduction).  The workspace is left dirty — the caller is responsible for
/// Bennett uncomputation of the entire forward pass.
///
/// When `is_square` is true, a and b point to the same register. All
/// recursive sub-calls also receive `is_square = true`, and the base case
/// uses `schoolbook_integer_square` (saving ~50% of partial-product Toffoli
/// via cross-term symmetry: each a[i]*a[j] pair computed once, doubled
/// by shifting position; diagonal terms use CNOT instead of Toffoli).
///
/// Gate count: O(n^1.585) Toffoli via 3 half-size sub-problems per level.
fn karatsuba_integer_mul(
    n: usize,
    a: usize,
    b: usize,
    acc: usize,
    ws: usize,
    is_square: bool,
    counter: &mut ResourceCounter,
) -> Vec<Gate> {
    // Base case: fall back to schoolbook
    if n <= 8 {
        if is_square {
            return schoolbook_integer_square(n, a, acc, ws, counter);
        } else {
            return schoolbook_integer_mul(n, a, b, acc, ws, counter);
        }
    }

    let h = n / 2;
    let h_hi = n - h; // handles odd n
    let h_sum = h.max(h_hi) + 1; // sa/sb register width (h_max + 1)

    let mut gates = Vec::new();

    // --- Workspace layout ---
    let z2_reg = ws;
    let sa_reg = z2_reg + 2 * h_hi;
    let sb_reg = sa_reg + h_sum;
    let z1_reg = sb_reg + h_sum;
    let carry_a = z1_reg + 2 * h_sum;
    let carry_b = carry_a + 1;
    let carry_comb = carry_b + 1;
    let sub_ws_base = carry_comb + 1;

    // Three separate sub-workspaces so dirty intermediates don't collide
    let sub_ws_0 = sub_ws_base;
    let sub_ws_1 = sub_ws_0 + karatsuba_int_ws(h);
    let sub_ws_2 = sub_ws_1 + karatsuba_int_ws(h_hi);

    // --- Step 1: z0 = a_lo * b_lo → acc[0..2h] ---
    let g = karatsuba_integer_mul(h, a, b, acc, sub_ws_0, is_square, counter);
    gates.extend(g);

    // --- Step 2: z2 = a_hi * b_hi → z2_reg[0..2*h_hi] ---
    let g = karatsuba_integer_mul(h_hi, a + h, b + h, z2_reg, sub_ws_1, is_square, counter);
    gates.extend(g);

    // --- Step 3: sa = a_lo + a_hi → sa_reg[0..h_sum] ---
    for i in 0..h {
        let g = Gate::Cnot {
            control: a + i,
            target: sa_reg + i,
        };
        counter.record_gate(&g);
        gates.push(g);
    }
    let g = add_with_carryout(h_hi, a + h, sa_reg, carry_a, counter);
    gates.extend(g);

    if is_square {
        // --- Step 4 (square): sb == sa, skip separate computation ---
        // Use sa_reg for both inputs to the z1 sub-call.

        // --- Step 5 (square): z1_full = sa² → z1_reg ---
        let g = karatsuba_integer_mul(h_sum, sa_reg, sa_reg, z1_reg, sub_ws_2, true, counter);
        gates.extend(g);
    } else {
        // --- Step 4: sb = b_lo + b_hi → sb_reg[0..h_sum] ---
        for i in 0..h {
            let g = Gate::Cnot {
                control: b + i,
                target: sb_reg + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = add_with_carryout(h_hi, b + h, sb_reg, carry_b, counter);
        gates.extend(g);

        // --- Step 5: z1_full = sa * sb → z1_reg[0..2*h_sum] ---
        let g = karatsuba_integer_mul(h_sum, sa_reg, sb_reg, z1_reg, sub_ws_2, false, counter);
        gates.extend(g);
    }

    // --- Step 6: z1 = z1_full - z0 - z2 (proper reversible subtraction) ---
    //
    // Karatsuba's middle term: z1 = (a_lo+a_hi)*(b_lo+b_hi) - z0 - z2
    //                             = a_lo*b_hi + a_hi*b_lo  (always >= 0).
    //
    // Reversible subtraction via Cuccaro-reverse: running the Cuccaro adder
    // gates in reverse order computes b -= a (undoing b += a). This costs
    // O(n) Toffoli per subtraction, matching proper integer arithmetic.
    //
    // z1_reg -= acc[0..2h] (subtract z0)
    let z0_bits = 2 * h;
    let sub_width_0 = z0_bits.min(2 * h_sum);
    let g = cuccaro_subtract(sub_width_0, acc, z1_reg, carry_comb, counter);
    gates.extend(g);

    // z1_reg -= z2_reg[0..2*h_hi] (subtract z2)
    let z2_bits = 2 * h_hi;
    let sub_width_2 = z2_bits.min(2 * h_sum);
    let g = cuccaro_subtract(sub_width_2, z2_reg, z1_reg, carry_comb, counter);
    gates.extend(g);

    // --- Step 7: Combine products into accumulator ---
    // acc[h..h+2*h_sum] += z1 (shifted by h positions)
    let z1_add_width = (2 * h_sum).min(2 * n - h);
    let z1_adder = CuccaroAdder::new(z1_add_width);
    let g = z1_adder.forward_gates(z1_reg, acc + h, carry_comb, counter);
    gates.extend(g);

    // acc[n..n+2*h_hi] += z2 (shifted by n positions)
    let z2_add_width = z2_bits.min(2 * n - n);
    let z2_adder = CuccaroAdder::new(z2_add_width);
    let g = z2_adder.forward_gates(z2_reg, acc + n, carry_comb, counter);
    gates.extend(g);

    gates
}

/// Reversible modular multiplier using Karatsuba decomposition.
///
/// Reduces per-multiply Toffoli count from O(n²) to O(n^1.585) by
/// recursively splitting n-bit operands into halves and performing 3
/// half-size multiplications instead of 4.
///
/// Structure:
/// 1. Forward: recursive Karatsuba integer multiply → 2n-bit product in acc
/// 2. Goldilocks modular reduction → n-bit result in acc[0..n]
/// 3. Copy result to output register
/// 4. Bennett uncomputation: reverse steps 1+2 to clean all workspace
///
/// Uses separate sub-workspaces for each sub-multiplication to allow
/// top-level-only Bennett uncomputation (no per-level Bennett overhead).
pub struct KaratsubaMultiplier {
    pub n: usize,
}

impl KaratsubaMultiplier {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Total workspace qubits needed: 2n (acc) + 1 (carry for reduction) +
    /// recursive Karatsuba workspace.
    pub fn workspace_size(n: usize) -> usize {
        2 * n + 1 + karatsuba_int_ws(n)
    }

    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // Workspace: acc(2n) + carry(1) + Karatsuba workspace
        let acc = workspace_offset;
        let carry = workspace_offset + 2 * n;
        let kara_ws = workspace_offset + 2 * n + 1;

        let total_ws = Self::workspace_size(n);
        counter.allocate_ancilla(total_ws);

        // --- Forward computation ---
        let mut forward_gates_list: Vec<Gate> = Vec::new();

        // Step 1: Integer multiplication via Karatsuba
        let g = karatsuba_integer_mul(n, a_offset, b_offset, acc, kara_ws, false, counter);
        forward_gates_list.extend(g);

        // Step 2: Goldilocks modular reduction
        let g = goldilocks_reduce(n, acc, carry, counter);
        forward_gates_list.extend(g);

        gates.extend(forward_gates_list.clone());

        // Step 3: Copy result to output register
        for i in 0..n {
            let g = Gate::Cnot {
                control: acc + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Step 4: Bennett uncomputation (reverse all forward gates)
        for gate in forward_gates_list.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(total_ws);
        gates
    }
}

/// Reversible squaring using Karatsuba decomposition.
pub struct KaratsubaSquarer {
    pub n: usize,
}

impl KaratsubaSquarer {
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
        let n = self.n;
        let mut gates = Vec::new();

        // Same workspace layout as KaratsubaMultiplier
        let acc = workspace_offset;
        let carry = workspace_offset + 2 * n;
        let kara_ws = workspace_offset + 2 * n + 1;

        let total_ws = KaratsubaMultiplier::workspace_size(n);
        counter.allocate_ancilla(total_ws);

        let mut forward_gates_list: Vec<Gate> = Vec::new();

        // Integer squaring via Karatsuba with is_square=true
        let g = karatsuba_integer_mul(n, input_offset, input_offset, acc, kara_ws, true, counter);
        forward_gates_list.extend(g);

        // Goldilocks modular reduction
        let g = goldilocks_reduce(n, acc, carry, counter);
        forward_gates_list.extend(g);

        gates.extend(forward_gates_list.clone());

        // Copy result to output
        for i in 0..n {
            let g = Gate::Cnot {
                control: acc + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Bennett uncomputation
        for gate in forward_gates_list.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(total_ws);
        gates
    }
}
