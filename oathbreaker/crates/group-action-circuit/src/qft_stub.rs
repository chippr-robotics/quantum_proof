use serde::{Deserialize, Serialize};

/// QFT resource estimate — described but not executed in v1.
///
/// The Quantum Fourier Transform on an n-qubit register requires O(n²) gates.
/// For the dual-register ECDLP formulation, QFT is applied independently to
/// both exponent registers (reg_a and reg_b) after the coherent group-action map.
///
/// Using the counting model implemented below
/// (Hadamards = n, controlled rotations = n(n-1)/2, SWAPs = n/2),
/// n=64 yields 2,112 gates per register and 4,224 total for two registers.
/// This is <0.1% of the EC arithmetic cost and requires no novel optimization.
///
/// QFT is included in resource projections and QASM export.
/// Execution is deferred to v2.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QftResourceEstimate {
    /// Number of qubits per register.
    pub num_qubits: usize,
    /// Number of registers QFT is applied to (2 for ECDLP).
    pub num_registers: usize,
    /// Hadamard gates per register.
    pub hadamard_count: usize,
    /// Controlled phase rotation gates per register.
    pub controlled_rotation_count: usize,
    /// SWAP gates per register (for bit reversal).
    pub swap_count: usize,
    /// Total gates across all registers.
    pub total_gates: usize,
}

impl QftResourceEstimate {
    /// Estimate QFT resources for a single n-qubit register.
    pub fn for_single_register(n: usize) -> Self {
        let hadamards = n;
        let rotations = n * (n - 1) / 2;
        let swaps = n / 2;
        Self {
            num_qubits: n,
            num_registers: 1,
            hadamard_count: hadamards,
            controlled_rotation_count: rotations,
            swap_count: swaps,
            total_gates: hadamards + rotations + swaps,
        }
    }

    /// Estimate QFT resources for dual registers (Shor's ECDLP).
    ///
    /// QFT is applied independently to both exponent registers.
    pub fn for_dual_register(n: usize) -> Self {
        let single = Self::for_single_register(n);
        Self {
            num_qubits: n,
            num_registers: 2,
            hadamard_count: single.hadamard_count * 2,
            controlled_rotation_count: single.controlled_rotation_count * 2,
            swap_count: single.swap_count * 2,
            total_gates: single.total_gates * 2,
        }
    }

    /// Format as a summary string.
    pub fn summary(&self) -> String {
        format!(
            "QFT Resource Estimate ({} register(s) x {} qubits):\n\
             ├── Hadamard gates:           {}\n\
             ├── Controlled rotations:     {}\n\
             ├── SWAP gates:               {}\n\
             └── Total QFT gates:          {}\n\
             Note: <0.1% of EC arithmetic cost. Deferred to v2.",
            self.num_registers,
            self.num_qubits,
            self.hadamard_count,
            self.controlled_rotation_count,
            self.swap_count,
            self.total_gates,
        )
    }
}
