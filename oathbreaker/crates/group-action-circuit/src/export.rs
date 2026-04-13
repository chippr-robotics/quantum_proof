use crate::double_scalar::GroupActionCircuit;
use crate::shor::ShorsEcdlp;
use reversible_arithmetic::gates::Gate;

/// Export the group-action circuit in OpenQASM 3.0 format.
///
/// The exported QASM can be loaded into:
/// - Qiskit (IBM)
/// - Cirq (Google)
/// - Other OpenQASM-compatible toolchains
///
/// This exports only the group-action map (stage 1 of Shor's algorithm).
/// For the complete Shor circuit including QFT and measurement, use
/// [`export_shor_qasm`].
pub fn export_qasm(circuit: &GroupActionCircuit) -> String {
    let mut qasm = String::new();

    qasm.push_str("OPENQASM 3.0;\n");
    qasm.push_str("include \"stdgates.inc\";\n\n");
    qasm.push_str("// Oathbreaker: coherent group-action circuit\n");
    qasm.push_str("// Computes |a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩\n");
    qasm.push_str(&format!(
        "// Field: GF(2^{} - 2^{} + 1), Window size: {}\n",
        circuit.curve.field_bits,
        circuit.curve.field_bits / 2,
        circuit.window_size
    ));
    qasm.push_str(&format!(
        "// Total qubits: {}, Toffoli gates: {}\n\n",
        circuit.qubit_count(),
        circuit.toffoli_count()
    ));

    qasm.push_str(&format!("qubit[{}] q;\n\n", circuit.qubit_count()));

    // Emit all reversible gates
    for gate in &circuit.gate_log {
        match gate {
            Gate::Not { target } => {
                qasm.push_str(&format!("x q[{}];\n", target));
            }
            Gate::Cnot { control, target } => {
                qasm.push_str(&format!("cx q[{}], q[{}];\n", control, target));
            }
            Gate::Toffoli {
                control1,
                control2,
                target,
            } => {
                qasm.push_str(&format!(
                    "ccx q[{}], q[{}], q[{}];\n",
                    control1, control2, target
                ));
            }
        }
    }

    // QFT resource annotation
    qasm.push_str(&format!(
        "\n// QFT gates: {} (Hadamard: {}, Rotations: {}, SWAPs: {})\n",
        circuit.qft_estimate.total_gates,
        circuit.qft_estimate.hadamard_count,
        circuit.qft_estimate.controlled_rotation_count,
        circuit.qft_estimate.swap_count,
    ));

    qasm
}

/// Export the complete Shor's ECDLP circuit in OpenQASM 3.0 format.
///
/// Includes all three stages:
/// 1. Group-action map (Toffoli/CNOT/NOT reversible gates)
/// 2. Inverse QFT on both exponent registers (Hadamard/CR/SWAP)
/// 3. Measurement of exponent registers
pub fn export_shor_qasm(shor: &ShorsEcdlp) -> String {
    let circuit = &shor.group_action_circuit;
    let n = shor.curve.field_bits;
    let mut qasm = String::new();

    qasm.push_str("OPENQASM 3.0;\n");
    qasm.push_str("include \"stdgates.inc\";\n\n");
    qasm.push_str(&format!(
        "// Oathbreaker: Complete Shor's ECDLP circuit (Oath-{})\n",
        n
    ));
    qasm.push_str("// Stages: Group-action map + Inverse QFT + Measurement\n");
    qasm.push_str(&format!(
        "// Window size: {}, Coordinate system: {}\n",
        shor.window_size, circuit.coordinate_system
    ));
    qasm.push_str(&format!(
        "// Total gates: {} (Toffoli: {}, QFT: {}, Measurement: {})\n\n",
        shor.gate_counts.total(),
        shor.gate_counts.toffoli,
        shor.gate_counts.hadamard + shor.gate_counts.controlled_phase + shor.gate_counts.swap,
        shor.gate_counts.measurement,
    ));

    // Quantum register
    qasm.push_str(&format!("qubit[{}] q;\n", circuit.qubit_count()));
    // Classical register for measurement results
    qasm.push_str(&format!("bit[{}] c;\n\n", 2 * n));

    // Stage 1: Group-action map
    qasm.push_str("// === Stage 1: Group-action map [a]G + [b]Q ===\n");
    for gate in &circuit.gate_log {
        match gate {
            Gate::Not { target } => qasm.push_str(&format!("x q[{}];\n", target)),
            Gate::Cnot { control, target } => {
                qasm.push_str(&format!("cx q[{}], q[{}];\n", control, target))
            }
            Gate::Toffoli {
                control1,
                control2,
                target,
            } => qasm.push_str(&format!(
                "ccx q[{}], q[{}], q[{}];\n",
                control1, control2, target
            )),
        }
    }

    // Stages 2–3: QFT + Measurement
    qasm.push_str("\n// === Stage 2: Inverse QFT on exponent registers ===\n");
    qasm.push_str("// === Stage 3: Measurement ===\n");
    for gate in &shor.qft_measurement_gates {
        qasm.push_str(&gate.to_qasm());
        qasm.push('\n');
    }

    qasm
}

/// Export circuit statistics as a JSON string for analysis tools.
pub fn export_stats_json(circuit: &GroupActionCircuit) -> String {
    serde_json::to_string_pretty(&circuit.summary()).unwrap_or_default()
}

/// Export complete Shor circuit statistics as JSON.
pub fn export_shor_stats_json(shor: &ShorsEcdlp) -> String {
    let summary = shor.group_action_circuit.summary();
    let gate_counts = &shor.gate_counts;

    let combined = serde_json::json!({
        "group_action": summary,
        "qft_gates": {
            "hadamard": gate_counts.hadamard,
            "controlled_phase": gate_counts.controlled_phase,
            "swap": gate_counts.swap,
        },
        "measurement_count": gate_counts.measurement,
        "total_gates": gate_counts.total(),
    });

    serde_json::to_string_pretty(&combined).unwrap_or_default()
}
