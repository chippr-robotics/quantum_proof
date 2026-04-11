use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::resource_counter::ResourceCounter;

/// Quantum Fourier Transform (classical description).
///
/// The QFT on an n-qubit register:
/// - O(n²) gates total (Hadamard + controlled phase rotations)
/// - For n=64: ~2,048 gates — trivial relative to the EC arithmetic
/// - Well-understood construction, no novel optimization needed
///
/// Note: The QFT uses Hadamard and controlled-rotation gates, which are
/// outside the {NOT, CNOT, Toffoli} gate set. For resource counting
/// purposes, we track them separately. The actual QFT only matters
/// on quantum hardware — for classical simulation, we describe its
/// effect on measurement probabilities.
pub struct QuantumFourierTransform {
    /// Number of qubits the QFT operates on.
    pub num_qubits: usize,
}

/// A gate that appears in the QFT but is outside the Toffoli gate set.
/// These are tracked separately for resource counting.
#[derive(Clone, Debug)]
pub enum QftGate {
    /// Hadamard gate on a single qubit.
    Hadamard { target: usize },
    /// Controlled phase rotation: CR_k = diag(1, 1, 1, e^{2πi/2^k})
    ControlledRotation {
        control: usize,
        target: usize,
        /// The rotation parameter k (rotation angle = 2π/2^k)
        k: usize,
    },
    /// SWAP gate (decomposable into 3 CNOTs)
    Swap { qubit1: usize, qubit2: usize },
}

impl QuantumFourierTransform {
    pub fn new(num_qubits: usize) -> Self {
        Self { num_qubits }
    }

    /// Generate the QFT gate sequence.
    ///
    /// Standard construction:
    /// ```text
    /// for i in 0..n:
    ///     H(q[i])
    ///     for j in (i+1)..n:
    ///         CR_{j-i+1}(q[j], q[i])
    /// // Reverse qubit order (swap)
    /// for i in 0..(n/2):
    ///     SWAP(q[i], q[n-1-i])
    /// ```
    pub fn gate_sequence(&self, register_offset: usize) -> Vec<QftGate> {
        let n = self.num_qubits;
        let mut gates = Vec::new();

        for i in 0..n {
            gates.push(QftGate::Hadamard {
                target: register_offset + i,
            });
            for j in (i + 1)..n {
                gates.push(QftGate::ControlledRotation {
                    control: register_offset + j,
                    target: register_offset + i,
                    k: j - i + 1,
                });
            }
        }

        // Reverse qubit order
        for i in 0..(n / 2) {
            gates.push(QftGate::Swap {
                qubit1: register_offset + i,
                qubit2: register_offset + n - 1 - i,
            });
        }

        gates
    }

    /// Resource count for the QFT.
    pub fn resource_count(&self) -> QftResources {
        let n = self.num_qubits;
        QftResources {
            hadamard_count: n,
            controlled_rotation_count: n * (n - 1) / 2,
            swap_count: n / 2,
            total_gates: n + n * (n - 1) / 2 + n / 2,
        }
    }
}

/// Resource summary for the QFT component.
#[derive(Clone, Debug)]
pub struct QftResources {
    pub hadamard_count: usize,
    pub controlled_rotation_count: usize,
    pub swap_count: usize,
    pub total_gates: usize,
}
