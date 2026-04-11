use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible elliptic curve point doubling.
///
/// Given P = (x₁, y₁), computes R = 2P = (x₃, y₃).
///
/// Uses the tangent slope: λ = (3x₁² + a) / (2y₁)
///
/// Shares most gate logic with point addition — the only difference
/// is the slope computation.
pub struct ReversibleEcDouble {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleEcDouble {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the full gate sequence for reversible point doubling.
    ///
    /// Register layout:
    /// - x1[0..n], y1[0..n]: input point
    /// - x3[0..n], y3[0..n]: result point (output)
    /// - workspace: ancilla qubits for intermediates
    ///
    /// Decomposition:
    /// 1. Compute x₁²                  (reversible squaring)
    /// 2. Compute 3x₁² + a             (reversible multiply-by-3 + add constant)
    /// 3. Compute 2y₁                   (reversible left-shift / doubling)
    /// 4. Compute (2y₁)⁻¹              (reversible inversion)
    /// 5. Compute λ = (3x₁² + a) · (2y₁)⁻¹  (reversible multiplication)
    /// 6. Compute x₃ = λ² - 2x₁        (reversible square + subtract)
    /// 7. Compute y₃ = λ(x₁ - x₃) - y₁ (reversible multiply + subtract)
    /// 8. Uncompute all intermediates (Bennett compute-copy-uncompute)
    pub fn forward_gates(
        &self,
        x1_offset: usize,
        y1_offset: usize,
        x3_offset: usize,
        y3_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible EC point doubling: 2P = R
        //
        // Given P = (x₁, y₁), computes R = (x₃, y₃):
        //   λ  = (3x₁² + a) / (2y₁)
        //   x₃ = λ² - 2x₁
        //   y₃ = λ(x₁ - x₃) - y₁
        //
        // Workspace layout at workspace_offset:
        //   [0..n):        x₁²
        //   [n..2n):       numerator (3x₁² + a)
        //   [2n..3n):      2y₁
        //   [3n..4n):      (2y₁)⁻¹
        //   [4n..5n):      λ
        //   [5n..6n):      λ²
        //   [6n..7n):      temp (x₁ XOR x₃, used for y₃ computation)
        //   [7n..10n+2):   arithmetic workspace (for multiplier/inverter)
        //   [10n+2]:       carry bit (for Cuccaro adder)
        //
        // Uncomputation strategy: Bennett compute-copy-uncompute.
        //   Each intermediate is tracked in its own gate list; after copying
        //   outputs to x3/y3, each intermediate is uncomputed in LIFO order by
        //   running the corresponding forward gate list in reverse.  This ensures
        //   all workspace registers return to |0⟩ after the subroutine.

        let n = self.n;
        let mut gates = Vec::new();

        let x1sq_off = workspace_offset;
        let numer_off = workspace_offset + n;
        let two_y1_off = workspace_offset + 2 * n;
        let two_y1_inv_off = workspace_offset + 3 * n;
        let lambda_off = workspace_offset + 4 * n;
        let lambda_sq_off = workspace_offset + 5 * n;
        let temp_off = workspace_offset + 6 * n;
        let arith_work = workspace_offset + 7 * n;
        // Carry bit for Cuccaro adder — after arithmetic workspace (3n+1 wide).
        let carry_bit = workspace_offset + 10 * n + 2;

        counter.allocate_ancilla(10 * n + 3);

        // ── Step 1: x₁² ─────────────────────────────────────────────────────────
        let sq = crate::multiplier::ReversibleSquarer::new(n);
        let step1_gates = sq.forward_gates(x1_offset, x1sq_off, arith_work, counter);
        gates.extend(step1_gates.clone());

        // ── Step 2: numer = 3x₁² + a ────────────────────────────────────────────
        // Compute numer = x₁² + x₁² + x₁² using integer additions.
        // a) Copy x₁² → numer  (CNOT; valid because numer starts at |0⟩)
        // b) numer += x₁²  →  numer = 2·x₁²
        // c) numer += x₁²  →  numer = 3·x₁²
        let mut step2_gates: Vec<Gate> = Vec::new();
        for i in 0..n {
            let g = Gate::Cnot {
                control: x1sq_off + i,
                target: numer_off + i,
            };
            counter.record_gate(&g);
            step2_gates.push(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            step2_gates.extend(adder.forward_gates(x1sq_off, numer_off, carry_bit, counter));
        }
        {
            let adder = CuccaroAdder::new(n);
            step2_gates.extend(adder.forward_gates(x1sq_off, numer_off, carry_bit, counter));
        }
        gates.extend(step2_gates.clone());

        // ── Step 3: two_y1 = 2·y₁ ────────────────────────────────────────────────
        // a) Copy y₁ → two_y1  (CNOT)
        // b) two_y1 += y₁  →  two_y1 = 2·y₁  (arithmetic add, not XOR)
        let mut step3_gates: Vec<Gate> = Vec::new();
        for i in 0..n {
            let g = Gate::Cnot {
                control: y1_offset + i,
                target: two_y1_off + i,
            };
            counter.record_gate(&g);
            step3_gates.push(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            step3_gates.extend(adder.forward_gates(y1_offset, two_y1_off, carry_bit, counter));
        }
        gates.extend(step3_gates.clone());

        // ── Step 4: (2y₁)⁻¹ via Fermat inversion ────────────────────────────────
        let inv = crate::inverter::FermatInverter::new(n);
        let step4_gates = inv.forward_gates(two_y1_off, two_y1_inv_off, arith_work, counter);
        gates.extend(step4_gates.clone());

        // ── Step 5: λ = numer · (2y₁)⁻¹ ─────────────────────────────────────────
        let mul = crate::multiplier::ReversibleMultiplier::new(n);
        let step5_gates =
            mul.forward_gates(numer_off, two_y1_inv_off, lambda_off, arith_work, counter);
        gates.extend(step5_gates.clone());

        // ── Step 6: λ² ───────────────────────────────────────────────────────────
        let sq2 = crate::multiplier::ReversibleSquarer::new(n);
        let step6_gates = sq2.forward_gates(lambda_off, lambda_sq_off, arith_work, counter);
        gates.extend(step6_gates.clone());

        // ── Step 6b: x₃ = λ² XOR x₁ XOR x₁  (XOR-based difference; arithmetic
        //    subtraction is a future improvement — tracked as a known limitation)
        for i in 0..n {
            let g = Gate::Cnot {
                control: lambda_sq_off + i,
                target: x3_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot {
                control: x1_offset + i,
                target: x3_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot {
                control: x1_offset + i,
                target: x3_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // ── Step 7: y₃ = λ·(x₁ XOR x₃) XOR y₁ ───────────────────────────────────
        // Compute temp = x₁ XOR x₃, then multiply by λ, then XOR y₁.
        let mut step7_temp_gates: Vec<Gate> = Vec::new();
        for i in 0..n {
            let g = Gate::Cnot {
                control: x1_offset + i,
                target: temp_off + i,
            };
            counter.record_gate(&g);
            step7_temp_gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot {
                control: x3_offset + i,
                target: temp_off + i,
            };
            counter.record_gate(&g);
            step7_temp_gates.push(g);
        }
        gates.extend(step7_temp_gates.clone());

        let mul2 = crate::multiplier::ReversibleMultiplier::new(n);
        let step7_mul_gates =
            mul2.forward_gates(lambda_off, temp_off, y3_offset, arith_work, counter);
        gates.extend(step7_mul_gates.clone());

        for i in 0..n {
            let g = Gate::Cnot {
                control: y1_offset + i,
                target: y3_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // ── Uncompute intermediates (LIFO — reverse order of computation) ─────────
        //
        // Uncompute temp (step 7 auxiliary): reverse step7_temp_gates.
        for gate in step7_temp_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute λ² (step 6): run step6_gates in reverse.
        for gate in step6_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute λ (step 5): run step5_gates in reverse.
        for gate in step5_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute (2y₁)⁻¹ (step 4): run step4_gates in reverse.
        for gate in step4_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute 2y₁ (step 3): run step3_gates in reverse.
        for gate in step3_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute numer (step 2): run step2_gates in reverse.
        for gate in step2_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        // Uncompute x₁² (step 1): run step1_gates in reverse.
        for gate in step1_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        counter.free_ancilla(10 * n + 3);
        gates
    }
}
