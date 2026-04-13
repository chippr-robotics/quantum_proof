use reversible_arithmetic::gates::Gate;
use serde::{Deserialize, Serialize};

/// Extended gate set for the full Shor's ECDLP quantum circuit.
///
/// The reversible arithmetic (Toffoli/CNOT/NOT) handles the group-action map,
/// while the QFT and measurement stages require non-classical quantum gates.
/// This enum wraps both under a single type for circuit serialization and
/// QASM export.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum QuantumGate {
    /// Classical reversible gate (NOT, CNOT, or Toffoli).
    Reversible(Gate),

    /// Hadamard gate: creates/destroys superposition.
    ///   |0⟩ → (|0⟩ + |1⟩) / √2
    ///   |1⟩ → (|0⟩ - |1⟩) / √2
    Hadamard { target: usize },

    /// Controlled phase rotation by angle 2π/2^k.
    ///   |1⟩|1⟩ → e^(2πi/2^k) |1⟩|1⟩
    /// Used in QFT for k = 2, 3, ..., n.
    /// The sign field distinguishes forward QFT (+1) from inverse QFT (-1).
    ControlledPhase {
        control: usize,
        target: usize,
        /// Rotation denominator exponent: angle = 2π / 2^k.
        k: usize,
        /// +1 for forward QFT, -1 for inverse QFT.
        sign: i8,
    },

    /// Swap two qubits. Used for QFT bit-reversal.
    Swap { qubit_a: usize, qubit_b: usize },

    /// Measurement in the computational basis.
    Measure { qubit: usize, classical_bit: usize },
}

impl QuantumGate {
    /// Return the set of qubit indices this gate operates on.
    pub fn qubits(&self) -> Vec<usize> {
        match self {
            QuantumGate::Reversible(g) => g.qubits(),
            QuantumGate::Hadamard { target } => vec![*target],
            QuantumGate::ControlledPhase {
                control, target, ..
            } => vec![*control, *target],
            QuantumGate::Swap { qubit_a, qubit_b } => vec![*qubit_a, *qubit_b],
            QuantumGate::Measure { qubit, .. } => vec![*qubit],
        }
    }

    /// Export this gate as an OpenQASM 3.0 statement.
    pub fn to_qasm(&self) -> String {
        match self {
            QuantumGate::Reversible(g) => match g {
                Gate::Not { target } => format!("x q[{}];", target),
                Gate::Cnot { control, target } => {
                    format!("cx q[{}], q[{}];", control, target)
                }
                Gate::Toffoli {
                    control1,
                    control2,
                    target,
                } => format!("ccx q[{}], q[{}], q[{}];", control1, control2, target),
            },
            QuantumGate::Hadamard { target } => format!("h q[{}];", target),
            QuantumGate::ControlledPhase {
                control,
                target,
                k,
                sign,
            } => {
                let sign_str = if *sign < 0 { "-" } else { "" };
                format!(
                    "cp({}2*pi/2**{}) q[{}], q[{}];",
                    sign_str, k, control, target
                )
            }
            QuantumGate::Swap { qubit_a, qubit_b } => {
                format!("swap q[{}], q[{}];", qubit_a, qubit_b)
            }
            QuantumGate::Measure {
                qubit,
                classical_bit,
            } => format!("c[{}] = measure q[{}];", classical_bit, qubit),
        }
    }
}

/// Aggregate gate counts for the full Shor circuit (reversible + quantum).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct QuantumGateCount {
    /// Classical reversible gates (from group-action map).
    pub toffoli: usize,
    pub cnot: usize,
    pub not: usize,
    /// QFT-specific gates.
    pub hadamard: usize,
    pub controlled_phase: usize,
    pub swap: usize,
    /// Measurement operations.
    pub measurement: usize,
}

impl QuantumGateCount {
    /// Count gates from a gate sequence.
    pub fn from_gates(gates: &[QuantumGate]) -> Self {
        let mut counts = Self::default();
        for gate in gates {
            counts.record(gate);
        }
        counts
    }

    /// Record a single gate.
    pub fn record(&mut self, gate: &QuantumGate) {
        match gate {
            QuantumGate::Reversible(g) => match g {
                Gate::Toffoli { .. } => self.toffoli += 1,
                Gate::Cnot { .. } => self.cnot += 1,
                Gate::Not { .. } => self.not += 1,
            },
            QuantumGate::Hadamard { .. } => self.hadamard += 1,
            QuantumGate::ControlledPhase { .. } => self.controlled_phase += 1,
            QuantumGate::Swap { .. } => self.swap += 1,
            QuantumGate::Measure { .. } => self.measurement += 1,
        }
    }

    /// Total gate count across all types.
    pub fn total(&self) -> usize {
        self.toffoli
            + self.cnot
            + self.not
            + self.hadamard
            + self.controlled_phase
            + self.swap
            + self.measurement
    }

    /// Format as a summary string.
    pub fn summary(&self) -> String {
        format!(
            "Quantum Gate Counts:\n\
             ├── Toffoli:            {}\n\
             ├── CNOT:               {}\n\
             ├── NOT:                {}\n\
             ├── Hadamard:           {}\n\
             ├── Controlled-Phase:   {}\n\
             ├── SWAP:               {}\n\
             ├── Measurement:        {}\n\
             └── Total:              {}",
            self.toffoli,
            self.cnot,
            self.not,
            self.hadamard,
            self.controlled_phase,
            self.swap,
            self.measurement,
            self.total(),
        )
    }
}
