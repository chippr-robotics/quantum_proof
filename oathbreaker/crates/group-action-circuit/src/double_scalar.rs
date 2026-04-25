use ec_oath::curve::CurveParams;
use reversible_arithmetic::ancilla::{AncillaPool, UncomputeStrategy};
use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::register::QuantumRegister;
use reversible_arithmetic::resource_counter::ResourceCounter;
use serde::{Deserialize, Serialize};

use crate::precompute::PrecomputeTable;
use crate::qft_stub::QftResourceEstimate;
use crate::scalar_mul::WindowedScalarMul;
use crate::scalar_mul_jacobian::WindowedScalarMulJacobian;
use crate::scalar_mul_jacobian_v3::WindowedScalarMulJacobianV3;

/// The coherent double-scalar group-action circuit: |a⟩|b⟩|O⟩ → |a⟩|b⟩|\[a\]G + \[b\]Q⟩
///
/// This is the computationally dominant component of Shor's ECDLP algorithm,
/// consuming >99% of the qubits and gates. The QFT adds only O(n²) gates
/// per register — trivial relative to the EC arithmetic.
///
/// For the complete Shor circuit (group-action + QFT + measurement),
/// use [`crate::shor::ShorsEcdlp::build`].
#[derive(Clone, Debug)]
pub struct GroupActionCircuit {
    /// Curve parameters (Oath-64).
    pub curve: CurveParams,
    /// Window size for scalar multiplication.
    pub window_size: usize,
    /// Coordinate system: "affine" or "jacobian".
    pub coordinate_system: String,
    /// Ordered log of all reversible gates (Toffoli/CNOT/NOT).
    pub gate_log: Vec<Gate>,
    /// QFT resource estimate (described but not executed in v1).
    pub qft_estimate: QftResourceEstimate,
    /// Overall resource summary.
    pub resources: ResourceCounter,
    /// Per-subsystem Toffoli breakdown (Jacobian circuits only).
    pub cost_attribution: Option<CostAttribution>,
}

/// Summary of the circuit's resource usage, suitable for publication.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CircuitSummary {
    pub field_bits: usize,
    pub window_size: usize,
    /// Coordinate system: "affine" or "jacobian"
    pub coordinate_system: String,
    pub logical_qubits_peak: usize,
    pub toffoli_gates: usize,
    pub cnot_gates: usize,
    pub not_gates: usize,
    pub total_reversible_gates: usize,
    pub circuit_depth: usize,
    pub ancilla_high_water: usize,
    /// QFT Hadamard gates (estimated from resource model).
    pub qft_hadamards_estimated: usize,
    /// QFT controlled-rotation gates (estimated from resource model).
    pub qft_rotations_estimated: usize,
    pub point_additions: usize,
    pub point_doublings: usize,
    pub field_inversions: usize,
    pub field_multiplications: usize,
    /// Per-subsystem Toffoli breakdown (populated for Jacobian circuits).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_attribution: Option<CostAttribution>,
    /// Estimated logical qubits with measurement-based uncomputation.
    ///
    /// With mid-circuit measurement and classical feedforward, workspace
    /// qubits can be measured and recycled after each EC operation.
    /// Only the primary registers (5n) plus the workspace for the single
    /// most expensive operation need to be alive simultaneously.
    ///
    /// This models the approach used by Google and Litinski to achieve
    /// ~1,200 logical qubits for 256-bit ECDLP.
    pub measurement_based_qubits: usize,
}

/// Per-subsystem Toffoli cost breakdown.
///
/// Populated by instrumenting the circuit builder with counter snapshots
/// at key phase boundaries.  Enables data-driven optimization targeting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CostAttribution {
    /// Toffoli from Jacobian point doublings (w per window × num_windows × 2 scalars).
    pub doubling_toffoli: usize,
    /// Toffoli from QROM decode/load/uncompute (per window × num_windows × 2 scalars).
    pub qrom_toffoli: usize,
    /// Toffoli from Jacobian mixed additions (1 per window × num_windows × 2 scalars).
    pub addition_toffoli: usize,
    /// Toffoli from register swaps/copies between EC ops.
    pub swap_toffoli: usize,
    /// Toffoli from the final inversion (Binary GCD or Fermat).
    pub inversion_toffoli: usize,
    /// Toffoli from affine recovery (Z⁻², Z⁻³, X·Z⁻², Y·Z⁻³).
    pub affine_recovery_toffoli: usize,
}

/// Build the coherent double-scalar group-action circuit.
///
/// This is the top-level assembly function that wires together:
/// 1. Two exponent registers (reg_a for G, reg_b for Q)
/// 2. Point accumulator register (x, y)
/// 3. Precomputed window tables for both G and Q
/// 4. Windowed scalar multiplication for \[a\]G
/// 5. Windowed scalar multiplication for \[b\]Q (added to accumulator)
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
        0,     // reg_a offset
        2 * n, // point_x offset
        3 * n, // point_y offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // Windowed scalar multiplication for [b]Q (adds to accumulator)
    let scalar_mul_b = WindowedScalarMul::new(window_size, n);
    let _gates_b = scalar_mul_b.forward_gates(
        n,     // reg_b offset
        2 * n, // point_x offset (same accumulator)
        3 * n, // point_y offset
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
        coordinate_system: "affine".to_string(),
        gate_log: Vec::new(),
        qft_estimate,
        resources: counter,
        cost_attribution: None,
    }
}

/// Build the coherent double-scalar group-action circuit using **Jacobian
/// projective coordinates** — the optimized variant.
///
/// Key difference from the affine version:
/// - Accumulator registers are (X, Y, Z) = 3n qubits instead of (x, y) = 2n
/// - All doublings and additions use Jacobian formulas: **0 inversions per EC op**
/// - A single Fermat inversion at the end converts Z back to affine
/// - Net effect: ~6× fewer Toffoli gates (inversion was 94% of affine cost)
///
/// Register layout:
///   exponent_a:  n qubits
///   exponent_b:  n qubits
///   point_X:     n qubits (Jacobian X)
///   point_Y:     n qubits (Jacobian Y)
///   point_Z:     n qubits (Jacobian Z)
///   Total primary: 5n qubits (vs 4n for affine — +n for Z register)
pub fn build_group_action_circuit_jacobian(
    curve: &CurveParams,
    window_size: usize,
) -> GroupActionCircuit {
    let n = curve.field_bits; // 64
    let mut counter = ResourceCounter::new();
    let mut ancilla_pool = AncillaPool::new(UncomputeStrategy::Eager);

    // Two exponent registers (dual-scalar formulation for ECDLP)
    let _reg_a = QuantumRegister::new("exponent_a", n);
    let _reg_b = QuantumRegister::new("exponent_b", n);

    // Point accumulator registers — Jacobian (X, Y, Z)
    let _point_x = QuantumRegister::new("point_X", n);
    let _point_y = QuantumRegister::new("point_Y", n);
    let _point_z = QuantumRegister::new("point_Z", n);

    // 5n primary qubits: two exponent registers + Jacobian point (X, Y, Z)
    counter.allocate_qubits(5 * n);

    // Precompute window tables for G (stays in affine — "mixed" addition)
    let _table_g = PrecomputeTable::generate_for_point(curve, &curve.generator, window_size);

    // Windowed Jacobian scalar multiplication for [a]G
    let scalar_mul_a = WindowedScalarMulJacobian::new(window_size, n);
    let (_gates_a, (dbl_a, qrom_a, add_a)) = scalar_mul_a.forward_gates(
        0,     // reg_a offset
        2 * n, // point_X offset
        3 * n, // point_Y offset
        4 * n, // point_Z offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // Reset pool — scalar_mul_a's ancillae (lookup regs, temp point, EC
    // workspace, one-hot register) are either uncomputed to |0⟩ (QROM,
    // lookup) or left dirty in workspace that will be overwritten by
    // scalar_mul_b (Bennett pattern).  Reusing these qubit indices avoids
    // inflating qubit_high_water by 2× the ancilla cost.
    ancilla_pool.reset_for_reuse(&mut counter);

    // Windowed Jacobian scalar multiplication for [b]Q (adds to accumulator)
    let scalar_mul_b = WindowedScalarMulJacobian::new(window_size, n);
    let (_gates_b, (dbl_b, qrom_b, add_b)) = scalar_mul_b.forward_gates(
        n,     // reg_b offset
        2 * n, // point_X offset (same accumulator)
        3 * n, // point_Y offset
        4 * n, // point_Z offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    // Track scalar mul subsystem costs
    let scalar_mul_doubling = dbl_a + dbl_b;
    let scalar_mul_qrom = qrom_a + qrom_b;
    let scalar_mul_addition = add_a + add_b;

    // Reset pool before inversion phase — scalar_mul workspace is no longer
    // needed and the inversion phase uses a different workspace layout.
    ancilla_pool.reset_for_reuse(&mut counter);

    // --- Single final inversion: convert Jacobian → affine ---
    let t_before_inv = counter.toffoli_count;
    let bgcd_ws_size = reversible_arithmetic::inverter::BinaryGcdInverter::workspace_size(n);
    let inv_workspace = ancilla_pool.allocate("final_inv_workspace", bgcd_ws_size, &mut counter);
    let z_inv_reg = ancilla_pool.allocate("z_inv", n, &mut counter);
    let inverter = reversible_arithmetic::inverter::BinaryGcdInverter::new(n);

    // BGCD inverter and multipliers use pre-allocated workspace — suppress
    // their internal allocate/free calls to avoid double-counting.
    counter.enter_pre_allocated();
    let _inv_gates = inverter.forward_gates(
        4 * n,            // point_Z input
        z_inv_reg.offset, // Z⁻¹ output
        inv_workspace.offset,
        &mut counter,
    );
    counter.exit_pre_allocated();

    let t_after_inv = counter.toffoli_count;

    // Compute Z⁻² = Z⁻¹ · Z⁻¹ and Z⁻³ = Z⁻² · Z⁻¹
    let z_inv2_reg = ancilla_pool.allocate("z_inv2", n, &mut counter);
    let z_inv3_reg = ancilla_pool.allocate("z_inv3", n, &mut counter);
    let mul = reversible_arithmetic::multiplier::KaratsubaMultiplier::new(n);
    let sq = reversible_arithmetic::multiplier::KaratsubaSquarer::new(n);

    // Squarer and multiplier calls reuse inv_workspace — suppress double-counting.
    counter.enter_pre_allocated();
    let _sq_gates = sq.forward_gates(
        z_inv_reg.offset,
        z_inv2_reg.offset,
        inv_workspace.offset,
        &mut counter,
    );
    let _mul_gates = mul.forward_gates(
        z_inv2_reg.offset,
        z_inv_reg.offset,
        z_inv3_reg.offset,
        inv_workspace.offset,
        &mut counter,
    );

    // x_affine = X · Z⁻²  (overwrite point_X with affine x)
    let _mul_x = mul.forward_gates(
        2 * n, // point_X
        z_inv2_reg.offset,
        2 * n, // result back to point_X
        inv_workspace.offset,
        &mut counter,
    );

    // y_affine = Y · Z⁻³  (overwrite point_Y with affine y)
    let _mul_y = mul.forward_gates(
        3 * n, // point_Y
        z_inv3_reg.offset,
        3 * n, // result back to point_Y
        inv_workspace.offset,
        &mut counter,
    );
    counter.exit_pre_allocated();

    let t_after_affine = counter.toffoli_count;

    // Build cost attribution
    let attribution = CostAttribution {
        doubling_toffoli: scalar_mul_doubling,
        qrom_toffoli: scalar_mul_qrom,
        addition_toffoli: scalar_mul_addition,
        swap_toffoli: 0, // CNOT swaps contribute 0 Toffoli
        inversion_toffoli: t_after_inv - t_before_inv,
        affine_recovery_toffoli: t_after_affine - t_after_inv,
    };

    // QFT resource estimate
    let qft_estimate = QftResourceEstimate::for_dual_register(n);

    GroupActionCircuit {
        curve: curve.clone(),
        window_size,
        coordinate_system: "jacobian".to_string(),
        gate_log: Vec::new(),
        qft_estimate,
        resources: counter,
        cost_attribution: Some(attribution),
    }
}

impl GroupActionCircuit {
    /// Get a publishable summary of circuit resources.
    pub fn summary(&self) -> CircuitSummary {
        let n = self.curve.field_bits;
        let num_windows = n / self.window_size;
        let w = self.window_size;
        let total_ec_ops = num_windows * (w + 1) * 2; // doublings + additions, ×2 scalars

        CircuitSummary {
            field_bits: self.curve.field_bits,
            window_size: self.window_size,
            coordinate_system: self.coordinate_system.clone(),
            logical_qubits_peak: self.resources.qubit_high_water,
            toffoli_gates: self.resources.toffoli_count,
            cnot_gates: self.resources.cnot_count,
            not_gates: self.resources.not_count,
            total_reversible_gates: self.resources.total_gates(),
            circuit_depth: self.resources.depth,
            ancilla_high_water: self.resources.ancilla_allocated,
            qft_hadamards_estimated: self.qft_estimate.hadamard_count,
            qft_rotations_estimated: self.qft_estimate.controlled_rotation_count,
            point_additions: num_windows * 2,
            point_doublings: num_windows * w * 2,
            field_inversions: if self.coordinate_system == "jacobian" {
                1
            } else {
                num_windows * 2
            },
            field_multiplications: total_ec_ops
                * if self.coordinate_system == "jacobian" {
                    11
                } else {
                    6
                },
            cost_attribution: self.cost_attribution.clone(),
            measurement_based_qubits: Self::estimate_measurement_based_qubits(n, w),
        }
    }

    /// Estimate logical qubits with measurement-based uncomputation.
    ///
    /// With mid-circuit measurement, workspace qubits are measured and
    /// recycled after each EC operation. The peak is the primary registers
    /// plus the workspace for the single most expensive concurrent operation.
    ///
    /// Peak phases:
    /// - Jacobian doubling: 5n (primary) + 3n (temp) + 14n+2 (workspace) = 22n+2
    /// - Mixed addition:    5n (primary) + 2n (lookup) + 3n (temp) + 13n+2 (ws) = 23n+2
    /// - QROM decode:       5n (primary) + 2n (lookup) + 2^w (one-hot) = 7n + 2^w
    /// - BGCD inversion:    5n (primary) + 5n+3 (BGCD ws) + n (Z⁻¹) = 11n+3
    ///
    /// The mixed addition phase dominates for typical window sizes (w ≤ 13).
    fn estimate_measurement_based_qubits(n: usize, w: usize) -> usize {
        let doubling_peak = 22 * n + 2;
        let addition_peak = 23 * n + 2;
        let qrom_peak = 7 * n + (1usize << w);
        let inversion_peak = 11 * n + 3;
        *[doubling_peak, addition_peak, qrom_peak, inversion_peak]
            .iter()
            .max()
            .unwrap()
    }

    /// Execute the circuit classically on specific basis-state inputs.
    ///
    /// Computes \[a\]G + \[b\]Q using the classical reference implementation.
    /// This is used for verification: the reversible circuit's classical execution
    /// must match this reference.
    pub fn execute_classical(
        &self,
        a: u64,
        b: u64,
        target_q: &ec_oath::AffinePoint,
    ) -> ec_oath::AffinePoint {
        let ag = ec_oath::point_ops::scalar_mul(a, &self.curve.generator, &self.curve);
        let bq = ec_oath::point_ops::scalar_mul(b, target_q, &self.curve);
        ec_oath::point_ops::point_add(&ag, &bq, &self.curve)
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

/// Build the V3 optimized double-scalar group-action circuit.
///
/// Optimizations over `build_group_action_circuit_jacobian`:
/// 1. **Modified Jacobian doubling**: caches aZ⁴ across doublings,
///    eliminating 2 squarings per doubling (12n+2 vs 14n+2 workspace).
/// 2. **Fixed ReversibleSquarer**: routes through KaratsubaSquarer
///    with symmetry optimization.
/// 3. **Reduced workspace**: tighter register scheduling.
///
/// Register layout:
///   exponent_a:  n qubits
///   exponent_b:  n qubits
///   point_X:     n qubits (Modified Jacobian X)
///   point_Y:     n qubits (Modified Jacobian Y)
///   point_Z:     n qubits (Modified Jacobian Z)
///   point_aZ4:   n qubits (Modified Jacobian aZ⁴)
///   Total primary: 6n qubits (vs 5n in v2 — +n for cached aZ⁴)
pub fn build_group_action_circuit_jacobian_v3(
    curve: &CurveParams,
    window_size: usize,
) -> GroupActionCircuit {
    let n = curve.field_bits;
    let mut counter = ResourceCounter::new();
    let mut ancilla_pool = AncillaPool::new(UncomputeStrategy::Eager);

    // Two exponent registers
    let _reg_a = QuantumRegister::new("exponent_a", n);
    let _reg_b = QuantumRegister::new("exponent_b", n);

    // Point accumulator — Modified Jacobian (X, Y, Z, aZ⁴)
    let _point_x = QuantumRegister::new("point_X", n);
    let _point_y = QuantumRegister::new("point_Y", n);
    let _point_z = QuantumRegister::new("point_Z", n);
    let _point_az4 = QuantumRegister::new("point_aZ4", n);

    // 6n primary qubits
    counter.allocate_qubits(6 * n);

    // Precompute table
    let _table_g = PrecomputeTable::generate_for_point(curve, &curve.generator, window_size);

    // V3 scalar multiplication for [a]G
    let scalar_mul_a = WindowedScalarMulJacobianV3::new(window_size, n);
    let (_gates_a, (dbl_a, qrom_a, add_a)) = scalar_mul_a.forward_gates(
        0,     // reg_a offset
        2 * n, // point_X offset
        3 * n, // point_Y offset
        4 * n, // point_Z offset
        5 * n, // point_aZ4 offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    ancilla_pool.reset_for_reuse(&mut counter);

    // V3 scalar multiplication for [b]Q
    let scalar_mul_b = WindowedScalarMulJacobianV3::new(window_size, n);
    let (_gates_b, (dbl_b, qrom_b, add_b)) = scalar_mul_b.forward_gates(
        n,     // reg_b offset
        2 * n, // point_X offset
        3 * n, // point_Y offset
        4 * n, // point_Z offset
        5 * n, // point_aZ4 offset
        &mut ancilla_pool,
        &mut counter,
        curve,
    );

    let scalar_mul_doubling = dbl_a + dbl_b;
    let scalar_mul_qrom = qrom_a + qrom_b;
    let scalar_mul_addition = add_a + add_b;

    ancilla_pool.reset_for_reuse(&mut counter);

    // --- Single final inversion: convert Jacobian → affine ---
    let t_before_inv = counter.toffoli_count;
    let bgcd_ws_size = reversible_arithmetic::inverter::BinaryGcdInverter::workspace_size(n);
    let inv_workspace = ancilla_pool.allocate("final_inv_workspace", bgcd_ws_size, &mut counter);
    let z_inv_reg = ancilla_pool.allocate("z_inv", n, &mut counter);
    let inverter = reversible_arithmetic::inverter::BinaryGcdInverter::new(n);

    counter.enter_pre_allocated();
    let _inv_gates = inverter.forward_gates(
        4 * n,            // point_Z input
        z_inv_reg.offset, // Z⁻¹ output
        inv_workspace.offset,
        &mut counter,
    );
    counter.exit_pre_allocated();

    let t_after_inv = counter.toffoli_count;

    // Compute Z⁻² = Z⁻¹ · Z⁻¹ and Z⁻³ = Z⁻² · Z⁻¹
    let z_inv2_reg = ancilla_pool.allocate("z_inv2", n, &mut counter);
    let z_inv3_reg = ancilla_pool.allocate("z_inv3", n, &mut counter);
    let mul = reversible_arithmetic::multiplier::KaratsubaMultiplier::new(n);
    let sq = reversible_arithmetic::multiplier::KaratsubaSquarer::new(n);

    counter.enter_pre_allocated();
    let _sq_gates = sq.forward_gates(
        z_inv_reg.offset,
        z_inv2_reg.offset,
        inv_workspace.offset,
        &mut counter,
    );
    let _mul_gates = mul.forward_gates(
        z_inv2_reg.offset,
        z_inv_reg.offset,
        z_inv3_reg.offset,
        inv_workspace.offset,
        &mut counter,
    );
    let _mul_x = mul.forward_gates(
        2 * n,
        z_inv2_reg.offset,
        2 * n,
        inv_workspace.offset,
        &mut counter,
    );
    let _mul_y = mul.forward_gates(
        3 * n,
        z_inv3_reg.offset,
        3 * n,
        inv_workspace.offset,
        &mut counter,
    );
    counter.exit_pre_allocated();

    let t_after_affine = counter.toffoli_count;

    let attribution = CostAttribution {
        doubling_toffoli: scalar_mul_doubling,
        qrom_toffoli: scalar_mul_qrom,
        addition_toffoli: scalar_mul_addition,
        swap_toffoli: 0,
        inversion_toffoli: t_after_inv - t_before_inv,
        affine_recovery_toffoli: t_after_affine - t_after_inv,
    };

    let qft_estimate = QftResourceEstimate::for_dual_register(n);

    GroupActionCircuit {
        curve: curve.clone(),
        window_size,
        coordinate_system: "jacobian-v3".to_string(),
        gate_log: Vec::new(),
        qft_estimate,
        resources: counter,
        cost_attribution: Some(attribution),
    }
}
