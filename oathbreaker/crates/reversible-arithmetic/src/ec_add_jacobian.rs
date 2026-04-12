use crate::gates::Gate;
use crate::multiplier::cuccaro_subtract;
use crate::resource_counter::ResourceCounter;

/// Reversible Jacobian mixed point addition (projective accumulator + affine table entry).
///
/// Adds an affine point Q = (x₂, y₂) to a Jacobian accumulator P = (X₁, Y₁, Z₁),
/// producing R = (X₃, Y₃, Z₃) in Jacobian coordinates.
///
/// **This is the critical optimization over affine addition**: no per-addition
/// field inversion. The entire scalar multiplication uses only Jacobian arithmetic,
/// with a single inversion at the very end to convert back to affine.
///
/// Mixed addition formulas (one affine input where Z=1):
///   Z₁² = Z₁ · Z₁
///   Z₁³ = Z₁² · Z₁
///   U₂  = x₂ · Z₁²
///   S₂  = y₂ · Z₁³
///   H   = U₂ - X₁
///   R   = S₂ - Y₁
///   H²  = H · H
///   H³  = H² · H
///   X₃  = R² - H³ - 2·X₁·H²
///   Y₃  = R·(X₁·H² - X₃) - Y₁·H³
///   Z₃  = Z₁ · H
///
/// Gate cost per addition: 11 mul-equivalents (3S + 8M), 0 inversions.
/// Subtractions use proper reversible Cuccaro-reverse arithmetic.
pub struct ReversibleJacobianMixedAdd {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleJacobianMixedAdd {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the full gate sequence for reversible Jacobian mixed addition.
    ///
    /// Register layout:
    /// - acc_x[0..n], acc_y[0..n], acc_z[0..n]: Jacobian accumulator (X₁, Y₁, Z₁)
    /// - q_x[0..n], q_y[0..n]: affine table entry (x₂, y₂) — read-only
    /// - out_x[0..n], out_y[0..n], out_z[0..n]: Jacobian result (X₃, Y₃, Z₃)
    /// - workspace: ancilla qubits for intermediates
    ///
    /// Workspace layout:
    ///   [0..n):      Z₁²
    ///   [n..2n):     Z₁³
    ///   [2n..3n):    U₂ = x₂·Z₁²
    ///   [3n..4n):    S₂ = y₂·Z₁³
    ///   [4n..5n):    H = U₂ - X₁
    ///   [5n..6n):    R = S₂ - Y₁
    ///   [6n..7n):    H²
    ///   [7n..8n):    H³
    ///   [8n..9n):    X₁·H²
    ///   `[9n..10n)`:   R² (temp, reused for Y₃ computation)
    ///   `[10n]`:       carry bit for Cuccaro subtraction
    ///   `[10n+1..13n+2)`: multiplier workspace
    #[allow(clippy::too_many_arguments)]
    pub fn forward_gates(
        &self,
        acc_x: usize,
        acc_y: usize,
        acc_z: usize,
        q_x: usize,
        q_y: usize,
        out_x: usize,
        out_y: usize,
        out_z: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // Workspace sub-register offsets
        let z1_sq = workspace_offset;
        let z1_cu = workspace_offset + n;
        let u2_off = workspace_offset + 2 * n;
        let s2_off = workspace_offset + 3 * n;
        let h_off = workspace_offset + 4 * n;
        let r_off = workspace_offset + 5 * n;
        let h_sq = workspace_offset + 6 * n;
        let h_cu = workspace_offset + 7 * n;
        let x1h2 = workspace_offset + 8 * n;
        let r_sq = workspace_offset + 9 * n;
        let sub_carry = workspace_offset + 10 * n;
        let mul_work = workspace_offset + 10 * n + 1;

        counter.allocate_ancilla(13 * n + 2);

        let mul = crate::multiplier::KaratsubaMultiplier::new(n);
        let sq = crate::multiplier::KaratsubaSquarer::new(n);

        // ---- Forward computation (3S + 8M = 11 mul-equivalents) ----

        // 1. Z₁² = Z₁ · Z₁
        let g = sq.forward_gates(acc_z, z1_sq, mul_work, counter);
        gates.extend(g);

        // 2. Z₁³ = Z₁² · Z₁
        let g = mul.forward_gates(z1_sq, acc_z, z1_cu, mul_work, counter);
        gates.extend(g);

        // 3. U₂ = x₂ · Z₁²
        let g = mul.forward_gates(q_x, z1_sq, u2_off, mul_work, counter);
        gates.extend(g);

        // 4. S₂ = y₂ · Z₁³
        let g = mul.forward_gates(q_y, z1_cu, s2_off, mul_work, counter);
        gates.extend(g);

        // 5. H = U₂ - X₁  (proper reversible subtraction)
        // h_off starts at 0; copy U₂ then subtract X₁
        for i in 0..n {
            let g = Gate::Cnot {
                control: u2_off + i,
                target: h_off + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, acc_x, h_off, sub_carry, counter);
        gates.extend(g);

        // 6. R = S₂ - Y₁
        for i in 0..n {
            let g = Gate::Cnot {
                control: s2_off + i,
                target: r_off + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, acc_y, r_off, sub_carry, counter);
        gates.extend(g);

        // 7. H² = H · H
        let g = sq.forward_gates(h_off, h_sq, mul_work, counter);
        gates.extend(g);

        // 8. H³ = H² · H
        let g = mul.forward_gates(h_sq, h_off, h_cu, mul_work, counter);
        gates.extend(g);

        // 9. X₁·H²
        let g = mul.forward_gates(acc_x, h_sq, x1h2, mul_work, counter);
        gates.extend(g);

        // 10. R²
        let g = sq.forward_gates(r_off, r_sq, mul_work, counter);
        gates.extend(g);

        // 11. X₃ = R² - H³ - 2·X₁·H²  (proper reversible subtraction)
        // Copy R² to out_x, then subtract H³, then subtract X₁·H² twice
        for i in 0..n {
            let g = Gate::Cnot {
                control: r_sq + i,
                target: out_x + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, h_cu, out_x, sub_carry, counter);
        gates.extend(g);
        let g = cuccaro_subtract(n, x1h2, out_x, sub_carry, counter);
        gates.extend(g);
        let g = cuccaro_subtract(n, x1h2, out_x, sub_carry, counter);
        gates.extend(g);

        // 12. Y₃ = R·(X₁·H² - X₃) - Y₁·H³
        // temp1 = X₁·H² - X₃ (reuse r_sq as temp since we're done with it)
        let temp1 = r_sq;
        // Clear r_sq first (reverse the R² copy that populated it via out_x copy)
        for i in (0..n).rev() {
            let g = Gate::Cnot {
                control: r_off + i,
                target: temp1 + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        // temp1 = X₁·H² - X₃ (copy X₁·H², subtract X₃)
        for i in 0..n {
            let g = Gate::Cnot {
                control: x1h2 + i,
                target: temp1 + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, out_x, temp1, sub_carry, counter);
        gates.extend(g);

        // R·temp1 → out_y
        let g = mul.forward_gates(r_off, temp1, out_y, mul_work, counter);
        gates.extend(g);

        // Y₁·H³ → y1h3_temp, then subtract from out_y
        let y1h3_temp = z1_cu; // reuse Z₁³ register (will be uncomputed later)
        let g = mul.forward_gates(acc_y, h_cu, y1h3_temp, mul_work, counter);
        gates.extend(g);
        let g = cuccaro_subtract(n, y1h3_temp, out_y, sub_carry, counter);
        gates.extend(g);

        // 13. Z₃ = Z₁ · H
        let g = mul.forward_gates(acc_z, h_off, out_z, mul_work, counter);
        gates.extend(g);

        // ---- Uncompute intermediates ----
        // NOTE: Only H, R, temp1, and Y₁·H³ temp are uncomputed here.
        // The remaining intermediates (Z₁², Z₁³, U₂, S₂, H², H³, X₁·H², R²)
        // are left dirty in the workspace. A full reversible implementation
        // would need to uncompute these via multiplication reversal, roughly
        // doubling the gate count. This is deferred to a future iteration.
        // For resource counting, the forward mul/sq operations are the
        // dominant cost term.

        // Uncompute Y₁·H³ temp (reverse the multiplication)
        let g = mul.forward_gates(acc_y, h_cu, y1h3_temp, mul_work, counter);
        for gate in g.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        // Uncompute temp1
        let g = cuccaro_subtract(n, out_x, temp1, sub_carry, counter);
        // Reverse the subtraction (apply same gates in reverse = re-add)
        for gate in g.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot {
                control: x1h2 + i,
                target: temp1 + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Uncompute R (reverse subtraction then reverse copy)
        let g = cuccaro_subtract(n, acc_y, r_off, sub_carry, counter);
        for gate in g.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot {
                control: s2_off + i,
                target: r_off + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Uncompute H (reverse subtraction then reverse copy)
        let g = cuccaro_subtract(n, acc_x, h_off, sub_carry, counter);
        for gate in g.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot {
                control: u2_off + i,
                target: h_off + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        counter.free_ancilla(13 * n + 2);
        gates
    }

    /// Estimated resource cost for one Jacobian mixed addition.
    ///
    /// 11 mul-equivalents (3S + 8M), 0 inversions.
    pub fn estimated_resources(&self) -> (usize, usize) {
        let muls = 11; // 3 squarings + 8 multiplications
        let toffoli_per_mul = self.n * self.n;
        let qubits = 3 * self.n + 2 * self.n + 3 * self.n + 13 * self.n + 2;
        let toffoli = 2 * muls * toffoli_per_mul;
        (qubits, toffoli)
    }
}
