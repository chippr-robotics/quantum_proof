use crate::double_scalar::GroupActionCircuit;
use reversible_arithmetic::gates::Gate;

/// Export the circuit in OpenQASM 3.0 format for quantum hardware execution.
///
/// The exported QASM can be loaded into:
/// - Qiskit (IBM)
/// - Cirq (Google)
/// - Other OpenQASM-compatible toolchains
///
/// Note: The QFT component (v2) uses Hadamard and controlled-rotation gates
/// which are natively supported in QASM. The reversible arithmetic
/// (Toffoli, CNOT, NOT) maps directly to QASM gates.
pub fn export_qasm(circuit: &GroupActionCircuit) -> String {
    let mut qasm = String::new();

    qasm.push_str("OPENQASM 3.0;\n");
    qasm.push_str("include \"stdgates.inc\";\n\n");
    qasm.push_str("// Oathbreaker: Oath-64 coherent group-action circuit\n");
    qasm.push_str("// Computes |a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩\n");
    qasm.push_str(&format!(
        "// Field: GF(2^64 - 2^32 + 1), Window size: {}\n",
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

    // QFT stub comment (v2)
    qasm.push_str("\n// QFT on exponent registers (deferred to v2)\n");
    qasm.push_str(&format!(
        "// Estimated QFT gates: {} (Hadamard: {}, Rotations: {})\n",
        circuit.qft_estimate.total_gates,
        circuit.qft_estimate.hadamard_count,
        circuit.qft_estimate.controlled_rotation_count,
    ));

    qasm
}

/// Export circuit statistics as a JSON string for analysis tools.
pub fn export_stats_json(circuit: &GroupActionCircuit) -> String {
    serde_json::to_string_pretty(&circuit.summary()).unwrap_or_default()
}
