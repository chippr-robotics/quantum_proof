use ec_goldilocks::curve::CurveParams;
use reversible_arithmetic::ancilla::{AncillaPool, UncomputeStrategy};
use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::register::QuantumRegister;
use reversible_arithmetic::resource_counter::ResourceCounter;
use serde::{Deserialize, Serialize};

use crate::precompute::PrecomputeTable;
use crate::qft_stub::QftResourceEstimate;
use crate::scalar_mul::WindowedScalarMul;

/// The coherent double-scalar group-action circuit: |a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩
///
/// This is the computationally dominant component of Shor's ECDLP algorithm,
/// consuming >99% of the qubits and gates. The QFT (applied in v2) adds
/// only O(n²) gates per register — trivial relative to the EC arithmetic.
#[derive(Clone, Debug)]
pub struct GroupActionCircuit {
    /// Curve parameters (Oath-64).
    pub curve: CurveParams,
    /// Window size for scalar multiplication.
    pub window_size: usize,
    /// Ordered log of all reversible gates (Toffoli/CNOT/NOT).
    pub gate_log: Vec<Gate>,
    /// QFT resource estimate (described but not executed in v1).
    pub qft_estimate: QftResourceEstimate,
    /// Overall resource summary.
    pub resources: ResourceCounter,
}

/// Summary of the circuit's resource usage, suitable for publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitSummary {
    pub field_bits: usize,
    pub window_size: usize,
    pub logical_qubits_peak: usize,
    pub toffoli_gates: usize,
    pub cnot_gates: usize,
    pub not_gates: usize,
    pub total_reversible_gates: usize,
    pub circuit_depth: usize,
    pub ancilla_high_water: usize,
    /// QFT resources (estimated, not executed in v1).
    pub qft_hadamards_estimated: usize,
    pub qft_rotations_estimated: usize,
    pub point_additions: usize,
    pub point_doublings: usize,
    pub field_inversions: usize,
    pub field_multiplications: usize,
}

/// Build the coherent double-scalar group-action circuit.
///
/// This is the top-level assembly function that wires together:
/// 1. Two exponent registers (reg_a for G, reg_b for Q)
/// 2. Point accumulator register (x, y)
/// 3. Precomputed window tables for both G and Q
/// 4. Windowed scalar multiplication for [a]G
/// 5. Windowed scalar multiplication for [b]Q (added to accumulator)
/// 6. QFT resource estimate (described, not executed in v1)
///
/// In the full Shor algorithm, QFT is applied to reg_a and reg_b
/// independently after this circuit. That is deferred to v2.
pub fn build_group_action_circuit(curve: &CurveParams, window_size: usize) -> GroupActionCircuit {
    let n = curve.field_bits; // 64
    let mut counter = ResourceCounter::new();
    let mut ancilla_pool = AncillaPool::new(UncomputeStrategy::Eager);

    // Two exponent registers (dual-scalar formulation for ECDLP)
    let _reg_a = QuantumRegister::new("exponent_a", n);
    let _reg_b = QuantumRegister::new("exponent_b", n);

    // Point accumulator registers
    let _point_x = QuantumRegister::new("point_x", n);
    let _point_y = QuantumRegister::new("point_y", n);

    // 4 * n primary qubits: two exponent registers + point (x, y)
    counter.allocate_qubits(4 * n);

    // Precompute window tables for G and Q
    let _table_g = PrecomputeTable::generate_for_point(curve, &curve.generator, window_size);
    // Q (target point) table would be generated at proof time with the specific instance

    // Windowed scalar multiplication for [a]G
    let scalar_mul_a = WindowedScalarMul::new(window_size, n);
    let _gates_a = scalar_mul_a.forward_gates(
        0,         // reg_a offset
        2 * n,     // point_x offset
        3 * n,     // point_y offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // Windowed scalar multiplication for [b]Q (adds to accumulator)
    let scalar_mul_b = WindowedScalarMul::new(window_size, n);
    let _gates_b = scalar_mul_b.forward_gates(
        n,         // reg_b offset
        2 * n,     // point_x offset (same accumulator)
        3 * n,     // point_y offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // QFT resource estimate (described, not executed in v1)
    // QFT on each n-qubit register: O(n²) gates per register
    let qft_estimate = QftResourceEstimate::for_dual_register(n);

    GroupActionCircuit {
        curve: curve.clone(),
        window_size,
        gate_log: Vec::new(), // populated by forward_gates once implemented
        qft_estimate,
        resources: counter,
    }
}

impl GroupActionCircuit {
    /// Get a publishable summary of circuit resources.
    pub fn summary(&self) -> CircuitSummary {
        CircuitSummary {
            field_bits: self.curve.field_bits,
            window_size: self.window_size,
            logical_qubits_peak: self.resources.qubit_high_water,
            toffoli_gates: self.resources.toffoli_count,
            cnot_gates: self.resources.cnot_count,
            not_gates: self.resources.not_count,
            total_reversible_gates: self.resources.total_gates(),
            circuit_depth: self.resources.depth,
            ancilla_high_water: self.resources.ancilla_allocated,
            qft_hadamards_estimated: self.qft_estimate.hadamard_count,
            qft_rotations_estimated: self.qft_estimate.controlled_rotation_count,
            point_additions: 0, // TODO: track during construction
            point_doublings: 0,
            field_inversions: 0,
            field_multiplications: 0,
        }
    }

    /// Execute the circuit classically on specific basis-state inputs.
    ///
    /// Computes [a]G + [b]Q using the classical reference implementation.
    /// This is used for verification: the reversible circuit's classical execution
    /// must match this reference.
    pub fn execute_classical(
        &self,
        a: u64,
        b: u64,
        target_q: &ec_goldilocks::AffinePoint,
    ) -> ec_goldilocks::AffinePoint {
        let ag = ec_goldilocks::point_ops::scalar_mul(a, &self.curve.generator, &self.curve);
        let bq = ec_goldilocks::point_ops::scalar_mul(b, target_q, &self.curve);
        ec_goldilocks::point_ops::point_add(&ag, &bq, &self.curve)
    }

    /// Total qubit count including ancillae.
    pub fn qubit_count(&self) -> usize {
        self.resources.qubit_high_water
    }

    /// Total Toffoli gate count.
    pub fn toffoli_count(&self) -> usize {
        self.resources.toffoli_count
    }

    /// Total CNOT gate count.
    pub fn cnot_count(&self) -> usize {
        self.resources.cnot_count
    }

    /// Circuit depth.
    pub fn depth(&self) -> usize {
        self.resources.depth
    }
}
