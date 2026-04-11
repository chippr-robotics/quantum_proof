use serde::{Deserialize, Serialize};

/// A reversible gate operating on qubit indices within the circuit.
///
/// All gates in this set are their own inverses or have an explicit
/// inverse defined (for uncomputation).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Gate {
    /// Pauli-X (NOT) gate: flips the target qubit.
    Not { target: usize },

    /// Controlled-NOT: flips target if control is |1⟩.
    Cnot { control: usize, target: usize },

    /// Toffoli (CCNOT): flips target if both controls are |1⟩.
    /// This is the universal reversible gate.
    Toffoli {
        control1: usize,
        control2: usize,
        target: usize,
    },
}

impl Gate {
    /// Return the inverse of this gate.
    /// NOT and CNOT are self-inverse. Toffoli is self-inverse.
    pub fn inverse(&self) -> Self {
        // All gates in our set are self-inverse
        self.clone()
    }

    /// Return the set of qubit indices this gate operates on.
    pub fn qubits(&self) -> Vec<usize> {
        match self {
            Gate::Not { target } => vec![*target],
            Gate::Cnot { control, target } => vec![*control, *target],
            Gate::Toffoli {
                control1,
                control2,
                target,
            } => vec![*control1, *control2, *target],
        }
    }

    /// Apply this gate to a classical bit vector (simulation).
    pub fn apply(&self, bits: &mut [bool]) {
        match self {
            Gate::Not { target } => {
                bits[*target] = !bits[*target];
            }
            Gate::Cnot { control, target } => {
                if bits[*control] {
                    bits[*target] = !bits[*target];
                }
            }
            Gate::Toffoli {
                control1,
                control2,
                target,
            } => {
                if bits[*control1] && bits[*control2] {
                    bits[*target] = !bits[*target];
                }
            }
        }
    }
}
