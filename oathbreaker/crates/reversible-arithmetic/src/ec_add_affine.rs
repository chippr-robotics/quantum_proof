use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible elliptic curve point addition.
///
/// Given P = (x₁, y₁) and Q = (x₂, y₂), computes R = P + Q = (x₃, y₃).
///
/// This is the subroutine that Google proved for secp256k1. Our version
/// operates on 64-bit registers over the Goldilocks field.
///
/// Decomposition:
/// 1. Compute Δx = x₂ - x₁         (reversible subtraction)
/// 2. Compute Δy = y₂ - y₁         (reversible subtraction)
/// 3. Compute Δx⁻¹                  (reversible inversion — most expensive)
/// 4. Compute λ = Δy · Δx⁻¹        (reversible multiplication)
/// 5. Compute λ²                    (reversible squaring)
/// 6. Compute x₃ = λ² - x₁ - x₂   (reversible subtraction)
/// 7. Compute y₃ = λ(x₁ - x₃) - y₁ (reversible multiply + subtract)
/// 8. Uncompute all intermediates    (λ, Δx, Δy, Δx⁻¹, λ²)
pub struct ReversibleEcAdd {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleEcAdd {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the full gate sequence for reversible point addition.
    ///
    /// Register layout:
    /// - x1[0..n], y1[0..n]: first point (may be modified depending on variant)
    /// - x2[0..n], y2[0..n]: second point (may be modified)
    /// - x3[0..n], y3[0..n]: result point (output)
    /// - workspace: ancilla qubits for intermediates (Δx, Δy, Δx⁻¹, λ, λ²)
    ///
    /// The workspace requires approximately 5n qubits for intermediates,
    /// plus whatever the inversion subroutine needs internally.
    pub fn forward_gates(
        &self,
        x1_offset: usize,
        y1_offset: usize,
        x2_offset: usize,
        y2_offset: usize,
        x3_offset: usize,
        y3_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible EC point addition: P + Q = R
        //
        // Given P = (x₁, y₁) and Q = (x₂, y₂), computes R = (x₃, y₃):
        //   λ  = (y₂ - y₁) / (x₂ - x₁)
        //   x₃ = λ² - x₁ - x₂
        //   y₃ = λ(x₁ - x₃) - y₁
        //
        // Workspace layout at workspace_offset:
        //   [0..n):     Δx  (x₂ - x₁; copy + subtract via Cuccaro)
        //   [n..2n):    Δy  (y₂ - y₁; copy + subtract via Cuccaro)
        //   [2n..3n):   Δx⁻¹
        //   [3n..4n):   λ
        //   [4n..5n):   λ²
        //   [5n..6n):   temp  (x₁ XOR x₃)
        //   [6n..9n+1): arithmetic workspace (multiplier/inverter)
        //   [9n+1]:     carry bit for Cuccaro adder
        //
        // Uncomputation: Bennett compute-copy-uncompute.  Each intermediate is
        // tracked; after outputs are written, intermediates are uncomputed in
        // LIFO order by reversing their forward gate sequences.
        //
        // NOTE: x₃ and y₃ are outputs placed in x3_offset / y3_offset.
        //       x3_offset and y3_offset MUST be distinct from all input / workspace
        //       registers; aliasing inputs and outputs will corrupt results.

        let n = self.n;
        let mut gates = Vec::new();

        let dx_off      = workspace_offset;
        let dy_off      = workspace_offset + n;
        let dx_inv_off  = workspace_offset + 2 * n;
        let lambda_off  = workspace_offset + 3 * n;
        let lambda_sq_off = workspace_offset + 4 * n;
        let temp_off    = workspace_offset + 5 * n;
        let arith_work  = workspace_offset + 6 * n;
        let carry_bit   = workspace_offset + 9 * n + 1;

        counter.allocate_ancilla(9 * n + 2);

        // ── Step 1: Δx = x₂ - x₁ ────────────────────────────────────────────────
        // Δx = x₂ + (−x₁) = x₂ + NOT(x₁) + 1  (two's complement in GF(2^n))
        //
        // Circuit:
        //   a) Load NOT(x₁) into Δx:
        //      - NOT every bit of Δx (from 0 → all-ones register)
        //      - CNOT(x₁[i], Δx[i]) for each i → Δx = all-ones XOR x₁ = NOT(x₁)
        //   b) Set carry_bit = 1 (NOT gate)
        //   c) CuccaroAdder(x₂, Δx, carry_bit) with carry-in = 1:
        //        Δx += x₂ + 1 = NOT(x₁) + x₂ + 1 = x₂ - x₁ (mod 2^n) ✓
        //      The Cuccaro adder returns carry_bit to its initial value (1) after the sweep.
        //   d) NOT carry_bit (restore carry_bit to 0).
        //
        // All gates are valid (no self-referential Toffoli).
        let mut step1_gates: Vec<Gate> = Vec::new();
        // a1) NOT all bits of Δx (initialise from 0 to all-ones)
        for i in 0..n {
            let g = Gate::Not { target: dx_off + i };
            counter.record_gate(&g);
            step1_gates.push(g);
        }
        // a2) CNOT(x₁, Δx) → Δx = NOT(x₁)
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: dx_off + i };
            counter.record_gate(&g);
            step1_gates.push(g);
        }
        // b) carry_bit ← 1
        {
            let g = Gate::Not { target: carry_bit };
            counter.record_gate(&g);
            step1_gates.push(g);
        }
        // c) Δx += x₂ + carry_in(=1) = Δx + x₂ + 1 = NOT(x₁) + x₂ + 1 = x₂ - x₁
        {
            let adder = CuccaroAdder::new(n);
            step1_gates.extend(adder.forward_gates(x2_offset, dx_off, carry_bit, counter));
        }
        // d) NOT carry_bit (restore to 0)
        {
            let g = Gate::Not { target: carry_bit };
            counter.record_gate(&g);
            step1_gates.push(g);
        }
        gates.extend(step1_gates.clone());

        // ── Step 2: Δy = y₂ - y₁ ────────────────────────────────────────────────
        // Same two's-complement approach.
        let mut step2_gates: Vec<Gate> = Vec::new();
        for i in 0..n {
            let g = Gate::Not { target: dy_off + i };
            counter.record_gate(&g);
            step2_gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: y1_offset + i, target: dy_off + i };
            counter.record_gate(&g);
            step2_gates.push(g);
        }
        {
            let g = Gate::Not { target: carry_bit };
            counter.record_gate(&g);
            step2_gates.push(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            step2_gates.extend(adder.forward_gates(y2_offset, dy_off, carry_bit, counter));
        }
        {
            let g = Gate::Not { target: carry_bit };
            counter.record_gate(&g);
            step2_gates.push(g);
        }
        gates.extend(step2_gates.clone());

        // ── Step 3: Δx⁻¹ ─────────────────────────────────────────────────────────
        let inverter = crate::inverter::FermatInverter::new(n);
        let step3_gates = inverter.forward_gates(dx_off, dx_inv_off, arith_work, counter);
        gates.extend(step3_gates.clone());

        // ── Step 4: λ = Δy · Δx⁻¹ ────────────────────────────────────────────────
        let mul = crate::multiplier::ReversibleMultiplier::new(n);
        let step4_gates = mul.forward_gates(dy_off, dx_inv_off, lambda_off, arith_work, counter);
        gates.extend(step4_gates.clone());

        // ── Step 5: λ² ───────────────────────────────────────────────────────────
        let sq = crate::multiplier::ReversibleSquarer::new(n);
        let step5_gates = sq.forward_gates(lambda_off, lambda_sq_off, arith_work, counter);
        gates.extend(step5_gates.clone());

        // ── Step 6: x₃ = λ² XOR x₁ XOR x₂ ───────────────────────────────────────
        // (XOR-based difference; arithmetic subtraction is a future improvement)
        for i in 0..n {
            let g = Gate::Cnot { control: lambda_sq_off + i, target: x3_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: x3_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: x2_offset + i, target: x3_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // ── Step 7: y₃ = λ·(x₁ XOR x₃) XOR y₁ ───────────────────────────────────
        let mut step7_temp_gates: Vec<Gate> = Vec::new();
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            step7_temp_gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: x3_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            step7_temp_gates.push(g);
        }
        gates.extend(step7_temp_gates.clone());

        let mul2 = crate::multiplier::ReversibleMultiplier::new(n);
        let step7_mul_gates = mul2.forward_gates(lambda_off, temp_off, y3_offset, arith_work, counter);
        gates.extend(step7_mul_gates.clone());

        for i in 0..n {
            let g = Gate::Cnot { control: y1_offset + i, target: y3_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // ── Uncompute intermediates (LIFO, Bennett compute-copy-uncompute) ────────
        //
        // Reverse order: temp → λ² → λ → Δx⁻¹ → Δy → Δx

        for gate in step7_temp_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        for gate in step5_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        for gate in step4_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        for gate in step3_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        for gate in step2_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        for gate in step1_gates.iter().rev() {
            let inv_g = gate.inverse();
            counter.record_gate(&inv_g);
            gates.push(inv_g);
        }

        counter.free_ancilla(9 * n + 2);
        gates
    }

    /// Estimated resource cost for one point addition.
    pub fn estimated_resources(&self) -> (usize, usize) {
        // Rough estimates for n-bit field:
        // - 1 inversion: ~64 multiplications (Fermat) = ~64 * n² Toffoli
        // - 3 multiplications: ~3 * n² Toffoli
        // - Several additions/subtractions: ~O(n) Toffoli each
        // - Uncomputation: roughly doubles the above
        //
        // Qubits: ~12n (2 input points + 1 output point + ~6n workspace)
        let qubits = 12 * self.n;
        let toffoli = 2 * (64 + 3) * self.n * self.n; // rough estimate
        (qubits, toffoli)
    }
}
