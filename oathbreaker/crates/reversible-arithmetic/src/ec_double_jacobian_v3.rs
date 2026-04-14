use crate::adder::CuccaroAdder;
use crate::gates::Gate;
use crate::multiplier::cuccaro_subtract;
use crate::resource_counter::ResourceCounter;

/// Reversible Modified Jacobian point doubling (v3 optimized).
///
/// Given P = (X₁, Y₁, Z₁, aZ₁⁴) in modified Jacobian coordinates,
/// computes 2P = (X₃, Y₃, Z₃, aZ₃⁴).
///
/// Key optimization over v2 (`ReversibleJacobianDouble`):
/// - Caches aZ⁴ across doublings, eliminating the Z₁²→Z₁⁴ chain
/// - Saves 2 squarings per doubling (Z₁² and Z₁⁴ no longer computed)
/// - Adds 1 multiplication for aZ₃⁴ update
/// - Net: -2S +1M per doubling = modest Toffoli reduction + workspace savings
/// - Workspace: 12n+2 (down from 14n+2)
///
/// Formulas (Cohen-Miyaji-Ono modified Jacobian):
///
/// - M  = 3·X₁² + aZ₁⁴  — 1S + const additions; aZ₁⁴ is cached
/// - Y₁²  — 1S
/// - X₁·Y₁² → const_temp  — 1M
/// - S  = 4·const_temp  — const additions
/// - M²  — 1S
/// - X₃ = M² - 2·S  — subtractions
/// - Y₁⁴ = Y₁²·Y₁²  — 1S
/// - T  = 8·Y₁⁴  — const additions
/// - Y₃ = M·(S - X₃) - T  — 1M + subtraction
/// - Z₃ = 2·Y₁·Z₁  — 1M + doubling
/// - aZ₃⁴ = 2·T·aZ₁⁴  — 1M + doubling
///
/// Total: 4M + 4S field ops (vs 3M + 6S in v2).
/// Net Toffoli: lower because 2 squarings eliminated, 1 mul added,
/// and squarings are only ~50% cheaper than muls in Karatsuba.
pub struct ReversibleJacobianDoubleV3 {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleJacobianDoubleV3 {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Workspace size: 12n + 2 qubits (down from 14n + 2 in v2).
    ///
    /// Layout (offset from workspace base):
    ///
    /// - `0..n` : X₁²
    /// - `n..2n` : M = 3·X₁² + aZ₁⁴
    /// - `2n..3n` : Y₁²
    /// - `3n..4n` : const_temp = X₁·Y₁² (kept dirty)
    /// - `4n..5n` : S = 4·X₁·Y₁²
    /// - `5n..6n` : M²
    /// - `6n..7n` : Y₁⁴
    /// - `7n..8n` : T = 8·Y₁⁴
    /// - `8n..9n` : temp for (S - X₃)
    /// - `9n` : sub_carry
    /// - `9n+1..` : multiplier workspace (~3n)
    pub fn workspace_size(n: usize) -> usize {
        12 * n + 2
    }

    /// Generate the full gate sequence for reversible modified Jacobian doubling.
    #[allow(clippy::too_many_arguments)]
    pub fn forward_gates(
        &self,
        in_x: usize,
        in_y: usize,
        in_z: usize,
        in_az4: usize,
        out_x: usize,
        out_y: usize,
        out_z: usize,
        out_az4: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // Workspace sub-register offsets
        let x1_sq = workspace_offset; // X₁²
        let m_off = workspace_offset + n; // M
        let y1_sq = workspace_offset + 2 * n; // Y₁²
        let const_temp = workspace_offset + 3 * n; // X₁·Y₁² (kept dirty)
        let s_off = workspace_offset + 4 * n; // S = 4·X₁·Y₁²
        let m_sq = workspace_offset + 5 * n; // M²
        let y1_4 = workspace_offset + 6 * n; // Y₁⁴
        let t_off = workspace_offset + 7 * n; // T = 8·Y₁⁴
        let temp = workspace_offset + 8 * n; // S - X₃
        let sub_carry = workspace_offset + 9 * n;
        let mul_work = workspace_offset + 9 * n + 1;

        counter.allocate_ancilla(Self::workspace_size(n));

        let mul = crate::multiplier::KaratsubaMultiplier::new(n);
        let sq = crate::multiplier::KaratsubaSquarer::new(n);

        // ---- Forward computation (4M + 4S) ----

        // 1. X₁² [1S]
        let g = sq.forward_gates(in_x, x1_sq, mul_work, counter);
        gates.extend(g);

        // 2. M = 3·X₁² + aZ₁⁴ [const additions]
        //    aZ₁⁴ comes from input — no Z₁², Z₁⁴ squarings needed!
        for _k in 0..3 {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(x1_sq, m_off, sub_carry, counter);
            gates.extend(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(in_az4, m_off, sub_carry, counter);
            gates.extend(g);
        }

        // 3. Y₁² [1S]
        let g = sq.forward_gates(in_y, y1_sq, mul_work, counter);
        gates.extend(g);

        // 4. const_temp = X₁·Y₁² [1M] — kept dirty (like v2)
        let g = mul.forward_gates(in_x, y1_sq, const_temp, mul_work, counter);
        gates.extend(g);

        // 5. S = 4·const_temp [const additions: add const_temp 4 times]
        for _k in 0..4 {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(const_temp, s_off, sub_carry, counter);
            gates.extend(g);
        }

        // 6. M² [1S]
        let g = sq.forward_gates(m_off, m_sq, mul_work, counter);
        gates.extend(g);

        // 7. X₃ = M² - 2·S [copy + 2 subtractions]
        for i in 0..n {
            let g = Gate::Cnot {
                control: m_sq + i,
                target: out_x + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, s_off, out_x, sub_carry, counter);
        gates.extend(g);
        let g = cuccaro_subtract(n, s_off, out_x, sub_carry, counter);
        gates.extend(g);

        // 8. Y₁⁴ = Y₁² · Y₁² [1S]
        let g = sq.forward_gates(y1_sq, y1_4, mul_work, counter);
        gates.extend(g);

        // 9. T = 8·Y₁⁴ [const additions: add y1_4 8 times]
        for _k in 0..8 {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(y1_4, t_off, sub_carry, counter);
            gates.extend(g);
        }

        // 10. Y₃ = M·(S - X₃) - T
        //     Compute S - X₃ into temp (temp is clean).
        for i in 0..n {
            let g = Gate::Cnot {
                control: s_off + i,
                target: temp + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        let g = cuccaro_subtract(n, out_x, temp, sub_carry, counter);
        gates.extend(g);

        //     M·(S - X₃) → out_y [1M]
        let g = mul.forward_gates(m_off, temp, out_y, mul_work, counter);
        gates.extend(g);

        //     out_y -= T
        let g = cuccaro_subtract(n, t_off, out_y, sub_carry, counter);
        gates.extend(g);

        // 11. Z₃ = 2·Y₁·Z₁ [1M + doubling]
        let g = mul.forward_gates(in_y, in_z, out_z, mul_work, counter);
        gates.extend(g);
        let dbl_scratch = mul_work; // reuse first n qubits of mul workspace
        for i in 0..n {
            let g = Gate::Cnot {
                control: out_z + i,
                target: dbl_scratch + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(dbl_scratch, out_z, sub_carry, counter);
            gates.extend(g);
        }

        // 12. aZ₃⁴ = 2·T·aZ₁⁴ [1M + doubling]
        let g = mul.forward_gates(t_off, in_az4, out_az4, mul_work, counter);
        gates.extend(g);
        for i in 0..n {
            let g = Gate::Cnot {
                control: out_az4 + i,
                target: dbl_scratch + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }
        {
            let adder = CuccaroAdder::new(n);
            let g = adder.forward_gates(dbl_scratch, out_az4, sub_carry, counter);
            gates.extend(g);
        }

        // ---- Partial uncompute ----
        // Clean temp (S - X₃) for reuse in next iteration
        let g = cuccaro_subtract(n, out_x, temp, sub_carry, counter);
        for gate in g.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }
        for i in (0..n).rev() {
            let g = Gate::Cnot {
                control: s_off + i,
                target: temp + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        counter.free_ancilla(Self::workspace_size(n));
        gates
    }

    /// Estimated resource cost for one modified Jacobian point doubling.
    pub fn estimated_resources(&self) -> (usize, usize) {
        // 4M + 4S = 8 field ops (vs 6S + 3M = 9 in v2)
        let muls = 8;
        let toffoli_per_mul = self.n * self.n;
        let qubits = 8 * self.n + Self::workspace_size(self.n);
        let toffoli = 2 * muls * toffoli_per_mul;
        (qubits, toffoli)
    }
}
