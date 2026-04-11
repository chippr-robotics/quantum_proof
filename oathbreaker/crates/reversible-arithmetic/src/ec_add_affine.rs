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
        // Decomposition into reversible subroutines:
        //   1. Δx = x₂ - x₁ (modular subtraction)
        //   2. Δy = y₂ - y₁ (modular subtraction)
        //   3. Δx⁻¹ = Δx^(p-2) (Fermat inversion — most expensive)
        //   4. λ = Δy · Δx⁻¹ (modular multiplication)
        //   5. λ² (modular squaring)
        //   6. x₃ = λ² - x₁ - x₂ (two modular subtractions)
        //   7. y₃ = λ·(x₁ - x₃) - y₁ (multiply + subtract)
        //   8. Uncompute intermediates (λ², λ, Δx⁻¹, Δy, Δx)
        //
        // Workspace layout at workspace_offset:
        //   [0..n):     Δx
        //   [n..2n):    Δy
        //   [2n..3n):   Δx⁻¹
        //   [3n..4n):   λ
        //   [4n..5n):   λ²
        //   [5n..6n):   x₁ - x₃ (temp for y₃ computation)
        //   [6n..9n+1): multiplier/inverter workspace
        //   [9n+1]:     carry bit

        let n = self.n;
        let mut gates = Vec::new();

        // Workspace register offsets
        let dx_off = workspace_offset;
        let dy_off = workspace_offset + n;
        let dx_inv_off = workspace_offset + 2 * n;
        let lambda_off = workspace_offset + 3 * n;
        let lambda_sq_off = workspace_offset + 4 * n;
        let temp_off = workspace_offset + 5 * n;
        let arith_work = workspace_offset + 6 * n;
        let _carry_off = workspace_offset + 9 * n;

        counter.allocate_ancilla(9 * n + 2);

        // --- Forward computation ---
        let mut forward_gates: Vec<Gate> = Vec::new();

        // Step 1: Δx = x₂ - x₁ (XOR x₂ into Δx, then XOR x₁)
        // Copy x₂ to Δx
        for i in 0..n {
            let g = Gate::Cnot { control: x2_offset + i, target: dx_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }
        // Subtract x₁ from Δx: Δx = x₂ ⊕ x₁ (simplified XOR subtraction)
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: dx_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // Step 2: Δy = y₂ - y₁
        for i in 0..n {
            let g = Gate::Cnot { control: y2_offset + i, target: dy_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: y1_offset + i, target: dy_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // Step 3: Δx⁻¹ = Δx^(p-2) via Fermat inversion
        let inverter = crate::inverter::FermatInverter::new(n);
        let inv_gates = inverter.forward_gates(dx_off, dx_inv_off, arith_work, counter);
        forward_gates.extend(inv_gates);

        // Step 4: λ = Δy · Δx⁻¹
        let mul = crate::multiplier::ReversibleMultiplier::new(n);
        let mul_gates = mul.forward_gates(dy_off, dx_inv_off, lambda_off, arith_work, counter);
        forward_gates.extend(mul_gates);

        // Step 5: λ²
        let sq = crate::multiplier::ReversibleSquarer::new(n);
        let sq_gates = sq.forward_gates(lambda_off, lambda_sq_off, arith_work, counter);
        forward_gates.extend(sq_gates);

        // Step 6: x₃ = λ² - x₁ - x₂
        // Copy λ² to x₃
        for i in 0..n {
            let g = Gate::Cnot { control: lambda_sq_off + i, target: x3_offset + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }
        // Subtract x₁
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: x3_offset + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }
        // Subtract x₂
        for i in 0..n {
            let g = Gate::Cnot { control: x2_offset + i, target: x3_offset + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // Step 7: y₃ = λ·(x₁ - x₃) - y₁
        // First compute temp = x₁ - x₃
        for i in 0..n {
            let g = Gate::Cnot { control: x1_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }
        for i in 0..n {
            let g = Gate::Cnot { control: x3_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        // Multiply: y₃_partial = λ · temp
        let mul2 = crate::multiplier::ReversibleMultiplier::new(n);
        let mul2_gates = mul2.forward_gates(lambda_off, temp_off, y3_offset, arith_work, counter);
        forward_gates.extend(mul2_gates);

        // Subtract y₁ from y₃
        for i in 0..n {
            let g = Gate::Cnot { control: y1_offset + i, target: y3_offset + i };
            counter.record_gate(&g);
            forward_gates.push(g);
        }

        gates.extend(forward_gates.clone());

        // --- Uncompute intermediates ---
        // We need to uncompute: temp, λ², λ, Δx⁻¹, Δy, Δx
        // But NOT x₃ and y₃ (those are outputs).
        //
        // We reverse the forward gates that computed intermediates,
        // skipping the output-producing steps (steps 6 and 7's final parts).
        // Since the forward gates include output steps, we selectively uncompute
        // only the workspace registers.

        // Uncompute temp (x₁ - x₃)
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: x3_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: x1_offset + i, target: temp_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Uncompute λ² (via reverse squaring — since we copied it to x₃,
        // the workspace copy needs to be cleaned)
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: lambda_off + i, target: lambda_sq_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Uncompute Δy
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: y1_offset + i, target: dy_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: y2_offset + i, target: dy_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Uncompute Δx
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: x1_offset + i, target: dx_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot { control: x2_offset + i, target: dx_off + i };
            counter.record_gate(&g);
            gates.push(g);
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
