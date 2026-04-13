use ec_goldilocks::curve::CurveParams;
use ec_goldilocks::AffinePoint;
use serde::{Deserialize, Serialize};

use crate::continued_fraction::{gcd, recover_secret_direct, recover_secret_multi};
use crate::double_scalar::{build_group_action_circuit_jacobian, GroupActionCircuit};
use crate::measurement::{sample_measurement_pairs, verify_measurement_pair};
use crate::qft::Qft;
use crate::quantum_gate::{QuantumGate, QuantumGateCount};

/// Complete Shor's ECDLP algorithm: circuit construction + classical recovery.
///
/// Shor's algorithm for the Elliptic Curve Discrete Logarithm Problem:
///   Given G (generator) and Q = [k]G, find the secret scalar k.
///
/// The quantum circuit has three stages:
/// 1. **Group-action map** (>99% of cost): |a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩
/// 2. **Inverse QFT**: applied to both exponent registers (a, b)
/// 3. **Measurement**: both exponent registers measured → (c, d)
///
/// Classical post-processing recovers k from (c, d) via:
///   k ≡ -c · d⁻¹ (mod r),  where r = #E(GF(p))
#[derive(Clone, Debug)]
pub struct ShorsEcdlp {
    /// Curve parameters (Oath-N tier).
    pub curve: CurveParams,
    /// Window size for scalar multiplication.
    pub window_size: usize,
    /// The group-action circuit (stage 1).
    pub group_action_circuit: GroupActionCircuit,
    /// QFT + measurement gate sequence (stages 2–3).
    pub qft_measurement_gates: Vec<QuantumGate>,
    /// Complete gate count across all stages.
    pub gate_counts: QuantumGateCount,
}

/// Result of running Shor's classical verification pipeline.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShorResult {
    /// The recovered secret scalar k.
    pub recovered_k: Option<u64>,
    /// Whether [k]G == Q was verified.
    pub verified: bool,
    /// Number of measurement trials used.
    pub num_trials: usize,
    /// Number of trials where direct inversion succeeded.
    pub direct_recovery_count: usize,
    /// Field bits of the curve.
    pub field_bits: usize,
    /// Total quantum gate count (all stages).
    pub total_gates: QuantumGateCount,
    /// Group-action circuit resource summary.
    pub group_action_toffoli: usize,
    /// QFT gate count (both registers).
    pub qft_gates: usize,
}

impl ShorsEcdlp {
    /// Build the complete Shor's ECDLP quantum circuit.
    ///
    /// Composes:
    /// 1. Jacobian group-action circuit for [a]G + [b]Q
    /// 2. Inverse QFT on both exponent registers
    /// 3. Measurement of both exponent registers
    pub fn build(curve: &CurveParams, window_size: usize) -> Self {
        let n = curve.field_bits;

        // Stage 1: Group-action map
        let group_action_circuit = build_group_action_circuit_jacobian(curve, window_size);

        // Stages 2–3: Inverse QFT + measurement on dual registers
        let qft_measurement_gates = Qft::shor_qft_and_measure(n);

        // Aggregate gate counts: start with reversible gates from group-action
        let mut gate_counts = QuantumGateCount {
            toffoli: group_action_circuit.resources.toffoli_count,
            cnot: group_action_circuit.resources.cnot_count,
            not: group_action_circuit.resources.not_count,
            ..Default::default()
        };

        // Add QFT + measurement gates
        for gate in &qft_measurement_gates {
            gate_counts.record(gate);
        }

        ShorsEcdlp {
            curve: curve.clone(),
            window_size,
            group_action_circuit,
            qft_measurement_gates,
            gate_counts,
        }
    }

    /// Run classical verification of Shor's algorithm.
    ///
    /// This simulates what a quantum computer would do:
    /// 1. Verify the group-action circuit produces correct [a]G + [b]Q
    /// 2. Simulate measurement outcomes (c, d) satisfying c + k·d ≡ 0 (mod r)
    /// 3. Run classical post-processing to recover k
    /// 4. Verify [k]G == Q
    ///
    /// Parameters:
    /// - `target_q`: The public point Q = [k]G
    /// - `k`: The secret scalar (used to generate measurement outcomes)
    /// - `num_trials`: Number of simulated measurement trials
    pub fn run_classical_verification(
        &self,
        target_q: &AffinePoint,
        k: u64,
        num_trials: usize,
    ) -> ShorResult {
        let order = self.curve.order;
        let n = self.curve.field_bits;

        // Step 1: Verify group-action circuit on sample inputs
        // Use a few random (a, b) pairs to confirm [a]G + [b]Q
        let test_scalars: Vec<(u64, u64)> = vec![(1, 0), (0, 1), (3, 5), (7, 11)];
        for (a, b) in &test_scalars {
            let result = self.group_action_circuit.execute_classical(*a, *b, target_q);
            let expected = ec_goldilocks::double_scalar_mul(
                *a,
                &self.curve.generator,
                *b,
                target_q,
                &self.curve,
            );
            debug_assert_eq!(
                result, expected,
                "Group-action mismatch for a={}, b={}",
                a, b
            );
        }

        // Step 2: Simulate quantum measurement outcomes
        let pairs = sample_measurement_pairs(k, order, num_trials);

        // Verify all pairs satisfy the Shor relation
        for (c, d) in &pairs {
            debug_assert!(
                verify_measurement_pair(*c, *d, k, order),
                "Invalid measurement pair"
            );
        }

        // Step 3: Classical post-processing — recover k
        let mut direct_count = 0;
        let mut recovered_k = None;

        // Try direct recovery on each pair
        for &(c, d) in &pairs {
            if gcd(d, order) == 1 {
                if let Some(candidate) = recover_secret_direct(c, d, order) {
                    direct_count += 1;
                    if recovered_k.is_none() {
                        recovered_k = Some(candidate);
                    }
                }
            }
        }

        // Fallback: multi-measurement recovery
        if recovered_k.is_none() {
            recovered_k = recover_secret_multi(&pairs, order);
        }

        // Step 4: Verify [k]G == Q
        let verified = if let Some(rk) = recovered_k {
            let computed_q = ec_goldilocks::point_ops::scalar_mul(
                rk,
                &self.curve.generator,
                &self.curve,
            );
            computed_q == *target_q
        } else {
            false
        };

        let qft_gate_count = self.qft_measurement_gates.len()
            - self.qft_measurement_gates.iter()
                .filter(|g| matches!(g, QuantumGate::Measure { .. }))
                .count();

        ShorResult {
            recovered_k,
            verified,
            num_trials,
            direct_recovery_count: direct_count,
            field_bits: n,
            total_gates: self.gate_counts.clone(),
            group_action_toffoli: self.group_action_circuit.resources.toffoli_count,
            qft_gates: qft_gate_count,
        }
    }

    /// Get a human-readable summary of the complete Shor circuit.
    pub fn summary(&self) -> String {
        let ga = self.group_action_circuit.summary();
        let n = self.curve.field_bits;
        let qft_per_reg = n + n * (n - 1) / 2 + n / 2;

        format!(
            "Shor's ECDLP Circuit (Oath-{}, w={}):\n\
             \n\
             Stage 1 — Group-Action Map [a]G + [b]Q:\n\
             ├── Coordinate system:     {}\n\
             ├── Logical qubits (peak): {}\n\
             ├── Toffoli gates:         {}\n\
             ├── CNOT gates:            {}\n\
             └── NOT gates:             {}\n\
             \n\
             Stage 2 — Inverse QFT (×2 registers):\n\
             ├── Hadamard gates:        {}\n\
             ├── Controlled-Phase:      {}\n\
             ├── SWAP gates:            {}\n\
             └── Gates per register:    {}\n\
             \n\
             Stage 3 — Measurement:\n\
             └── Measured qubits:       {} (2 × {} registers)\n\
             \n\
             Total Gates:               {}",
            n,
            self.window_size,
            ga.coordinate_system,
            ga.logical_qubits_peak,
            ga.toffoli_gates,
            ga.cnot_gates,
            ga.not_gates,
            self.gate_counts.hadamard,
            self.gate_counts.controlled_phase,
            self.gate_counts.swap,
            qft_per_reg,
            2 * n,
            n,
            self.gate_counts.total(),
        )
    }
}
