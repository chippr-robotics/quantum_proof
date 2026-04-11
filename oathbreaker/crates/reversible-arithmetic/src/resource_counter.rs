use crate::gates::Gate;
use serde::{Deserialize, Serialize};

/// Tracks circuit resource usage: qubits, gates, and depth.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResourceCounter {
    /// Total number of NOT gates.
    pub not_count: usize,
    /// Total number of CNOT gates.
    pub cnot_count: usize,
    /// Total number of Toffoli gates.
    pub toffoli_count: usize,
    /// Peak number of simultaneously active qubits.
    pub qubit_high_water: usize,
    /// Current number of active qubits.
    pub current_qubits: usize,
    /// Circuit depth (critical path length).
    pub depth: usize,
    /// Number of ancilla qubits allocated.
    pub ancilla_allocated: usize,
    /// Number of ancilla qubits successfully uncomputed.
    pub ancilla_freed: usize,
}

impl ResourceCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a gate application.
    pub fn record_gate(&mut self, gate: &Gate) {
        match gate {
            Gate::Not { .. } => self.not_count += 1,
            Gate::Cnot { .. } => self.cnot_count += 1,
            Gate::Toffoli { .. } => self.toffoli_count += 1,
        }
        self.depth += 1; // Simplified: assumes sequential execution
    }

    /// Record allocation of qubits.
    pub fn allocate_qubits(&mut self, count: usize) {
        self.current_qubits += count;
        if self.current_qubits > self.qubit_high_water {
            self.qubit_high_water = self.current_qubits;
        }
    }

    /// Record allocation of ancilla qubits.
    pub fn allocate_ancilla(&mut self, count: usize) {
        self.ancilla_allocated += count;
        self.allocate_qubits(count);
    }

    /// Record freeing of ancilla qubits (after uncomputation).
    pub fn free_ancilla(&mut self, count: usize) {
        self.ancilla_freed += count;
        self.current_qubits = self.current_qubits.saturating_sub(count);
    }

    /// Total gate count.
    pub fn total_gates(&self) -> usize {
        self.not_count + self.cnot_count + self.toffoli_count
    }

    /// Print a summary table.
    pub fn summary(&self) -> String {
        format!(
            "Circuit Resources:\n\
             ├── Logical qubits (peak): {}\n\
             ├── Toffoli gates:         {}\n\
             ├── CNOT gates:            {}\n\
             ├── NOT gates:             {}\n\
             ├── Total gates:           {}\n\
             ├── Circuit depth:         {}\n\
             ├── Ancilla allocated:     {}\n\
             └── Ancilla freed:         {}",
            self.qubit_high_water,
            self.toffoli_count,
            self.cnot_count,
            self.not_count,
            self.total_gates(),
            self.depth,
            self.ancilla_allocated,
            self.ancilla_freed,
        )
    }
}
