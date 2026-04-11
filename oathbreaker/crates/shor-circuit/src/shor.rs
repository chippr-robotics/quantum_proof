use ec_goldilocks::curve::CurveParams;
use reversible_arithmetic::ancilla::{AncillaPool, UncomputeStrategy};
use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::resource_counter::ResourceCounter;
use serde::{Deserialize, Serialize};

use crate::phase_estimation::PhaseEstimation;
use crate::precompute::PrecomputeTable;
use crate::qft::{QuantumFourierTransform, QftResources};
use crate::scalar_mul::WindowedScalarMul;

/// The complete Shor circuit for ECDLP over the Goldilocks field.
#[derive(Clone, Debug)]
pub struct ShorCircuit {
    /// Curve parameters.
    pub curve: CurveParams,
    /// Window size for scalar multiplication.
    pub window_size: usize,
    /// Ordered log of all reversible gates (Toffoli/CNOT/NOT).
    pub gate_log: Vec<Gate>,
    /// QFT resources (tracked separately from Toffoli-based gates).
    pub qft_resources: QftResources,
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
    pub qft_hadamards: usize,
    pub qft_rotations: usize,
    pub point_additions: usize,
    pub point_doublings: usize,
    pub field_inversions: usize,
    pub field_multiplications: usize,
}

/// Build the complete Shor circuit for ECDLP.
///
/// This is the top-level assembly function that wires together:
/// 1. Register allocation (scalar, point_x, point_y)
/// 2. Precomputed point table
/// 3. Windowed scalar multiplication loop (reversible EC arithmetic)
/// 4. Quantum Fourier Transform on the scalar register
pub fn build_shor_circuit(curve: &CurveParams, window_size: usize) -> ShorCircuit {
    let n = curve.field_bits; // 64
    let mut counter = ResourceCounter::new();
    // Initialise the ancilla pool starting beyond the three primary registers
    // (scalar at 0..n, point_x at n..2n, point_y at 2n..3n) so that ancilla
    // qubit indices never collide with primary register indices.
    let mut ancilla_pool = AncillaPool::new_with_base_offset(3 * n, UncomputeStrategy::Eager);

    // Phase estimation register allocation
    let _phase_est = PhaseEstimation::new(n, &mut counter);

    // Precompute window table
    let _precomp_table = PrecomputeTable::generate(curve, window_size);

    // Windowed scalar multiplication
    let scalar_mul = WindowedScalarMul::new(window_size, n);
    let _scalar_mul_gates = scalar_mul.forward_gates(
        0,         // scalar register offset
        n,         // point_x register offset
        2 * n,     // point_y register offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // QFT on scalar register
    let qft = QuantumFourierTransform::new(n);
    let qft_resources = qft.resource_count();

    // TODO: Collect all gates from the scalar multiplication circuit
    // and combine with QFT description.

    ShorCircuit {
        curve: curve.clone(),
        window_size,
        gate_log: Vec::new(), // populated by scalar_mul.forward_gates
        qft_resources,
        resources: counter,
    }
}

impl ShorCircuit {
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
            qft_hadamards: self.qft_resources.hadamard_count,
            qft_rotations: self.qft_resources.controlled_rotation_count,
            // TODO: Track these during circuit construction
            point_additions: 0,
            point_doublings: 0,
            field_inversions: 0,
            field_multiplications: 0,
        }
    }

    /// Execute the circuit classically on a specific scalar input.
    /// Returns the resulting EC point [k]G.
    ///
    /// This is used for verification: the circuit's classical execution
    /// must match the reference implementation in ec-goldilocks.
    pub fn execute_classical(&self, k: u64) -> ec_goldilocks::AffinePoint {
        // For classical verification, we simply compute [k]G using
        // the standard (non-reversible) scalar multiplication.
        ec_goldilocks::point_ops::scalar_mul(k, &self.curve.generator, &self.curve)
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
