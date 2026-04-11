use crate::gates::Gate;
use crate::register::QuantumRegister;
use crate::resource_counter::ResourceCounter;

/// Manages ancilla qubit allocation and uncomputation.
///
/// Two strategies are supported:
/// - **Eager uncomputation**: Free ancillae as soon as they're no longer needed.
///   Minimizes qubit count but increases gate count.
/// - **Deferred uncomputation (Bennett's pebble game)**: Keep intermediates alive
///   and uncompute in bulk later. Minimizes gates but uses more qubits.
///
/// The choice determines the qubit/gate tradeoff curve — this is one of the main
/// things Google optimized in their withheld circuits.
pub struct AncillaPool {
    /// All allocated ancilla registers.
    registers: Vec<QuantumRegister>,
    /// The global qubit index at which ancilla allocation begins.
    /// Ancilla indices start here to avoid colliding with primary registers.
    base_offset: usize,
    /// Next available qubit index (>= base_offset).
    next_qubit: usize,
    /// Strategy for uncomputation.
    pub strategy: UncomputeStrategy,
    /// Deferred uncomputation: gates to reverse later.
    deferred_gates: Vec<Vec<Gate>>,
}

/// Strategy for ancilla uncomputation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UncomputeStrategy {
    /// Uncompute intermediate values as soon as they're no longer needed.
    /// Saves qubits, costs gates.
    Eager,
    /// Defer uncomputation using Bennett's pebble game strategy.
    /// Saves gates, costs qubits.
    Deferred,
}

impl AncillaPool {
    pub fn new(strategy: UncomputeStrategy) -> Self {
        Self {
            registers: Vec::new(),
            base_offset: 0,
            next_qubit: 0,
            strategy,
            deferred_gates: Vec::new(),
        }
    }

    /// Create a new pool whose qubit indices start at `base_offset`,
    /// avoiding collisions with already-allocated primary registers.
    pub fn new_with_base_offset(base_offset: usize, strategy: UncomputeStrategy) -> Self {
        Self {
            registers: Vec::new(),
            base_offset,
            next_qubit: base_offset,
            strategy,
            deferred_gates: Vec::new(),
        }
    }

    /// Allocate a new ancilla register.
    pub fn allocate(
        &mut self,
        label: &str,
        num_bits: usize,
        counter: &mut ResourceCounter,
    ) -> QuantumRegister {
        let mut reg = QuantumRegister::new_ancilla(label, num_bits);
        reg.offset = self.next_qubit;
        self.next_qubit += num_bits;
        counter.allocate_ancilla(num_bits);
        self.registers.push(reg.clone());
        reg
    }

    /// Record gates that will need to be reversed for uncomputation.
    pub fn record_for_uncompute(&mut self, gates: Vec<Gate>) {
        self.deferred_gates.push(gates);
    }

    /// Generate uncomputation gates for all deferred computations.
    /// Gates are reversed in LIFO order (last computed = first uncomputed).
    pub fn flush_uncompute(&mut self, counter: &mut ResourceCounter) -> Vec<Gate> {
        let mut uncompute_gates = Vec::new();
        while let Some(forward_gates) = self.deferred_gates.pop() {
            for gate in forward_gates.iter().rev() {
                let inv = gate.inverse();
                counter.record_gate(&inv);
                uncompute_gates.push(inv);
            }
        }
        uncompute_gates
    }

    /// Total ancilla qubits currently allocated (excludes the base offset).
    pub fn total_allocated(&self) -> usize {
        self.next_qubit - self.base_offset
    }
}
