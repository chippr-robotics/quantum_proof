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
    /// Nesting depth of pre-allocated workspace scopes.
    /// When > 0, inner allocate_ancilla/free_ancilla calls skip
    /// qubit counting (the workspace is already counted by the outer scope).
    #[serde(skip)]
    pub pre_allocated_depth: usize,
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
    ///
    /// When inside a pre-allocated workspace scope (see [`enter_pre_allocated`]),
    /// the allocation is recorded for bookkeeping but does NOT affect
    /// `current_qubits` or `qubit_high_water`, since the outer scope already
    /// counted those qubits.
    pub fn allocate_ancilla(&mut self, count: usize) {
        self.ancilla_allocated += count;
        if self.pre_allocated_depth == 0 {
            self.allocate_qubits(count);
        }
    }

    /// Record freeing of ancilla qubits (after uncomputation).
    ///
    /// When inside a pre-allocated workspace scope, the free is recorded
    /// for bookkeeping but does NOT affect `current_qubits`.
    pub fn free_ancilla(&mut self, count: usize) {
        self.ancilla_freed += count;
        if self.pre_allocated_depth == 0 {
            self.current_qubits = self.current_qubits.saturating_sub(count);
        }
    }

    /// Enter a pre-allocated workspace scope.
    ///
    /// Call this before invoking operations that use workspace already
    /// allocated by the caller (e.g., EC point operations using workspace
    /// pre-allocated by the scalar multiplication loop). Inner operations'
    /// `allocate_ancilla` / `free_ancilla` calls will be suppressed for
    /// qubit counting purposes, since the workspace is already accounted for.
    ///
    /// Scopes nest: call `exit_pre_allocated` once for each `enter_pre_allocated`.
    pub fn enter_pre_allocated(&mut self) {
        self.pre_allocated_depth += 1;
    }

    /// Exit a pre-allocated workspace scope.
    pub fn exit_pre_allocated(&mut self) {
        self.pre_allocated_depth = self.pre_allocated_depth.saturating_sub(1);
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
