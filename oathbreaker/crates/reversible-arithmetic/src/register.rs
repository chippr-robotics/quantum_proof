use serde::{Deserialize, Serialize};

/// A quantum register abstraction for classical simulation.
///
/// Each register holds a fixed number of bits and tracks whether it
/// represents ancilla qubits that will need uncomputation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumRegister {
    /// Current classical state of each qubit.
    pub bits: Vec<bool>,
    /// Human-readable label for this register.
    pub label: String,
    /// Whether this register holds ancilla qubits.
    pub is_ancilla: bool,
    /// Global qubit index offset (position within the full circuit).
    pub offset: usize,
}

impl QuantumRegister {
    /// Create a new register initialized to all zeros.
    pub fn new(label: &str, num_bits: usize) -> Self {
        Self {
            bits: vec![false; num_bits],
            label: label.to_string(),
            is_ancilla: false,
            offset: 0,
        }
    }

    /// Create a new ancilla register initialized to all zeros.
    pub fn new_ancilla(label: &str, num_bits: usize) -> Self {
        Self {
            bits: vec![false; num_bits],
            label: label.to_string(),
            is_ancilla: true,
            offset: 0,
        }
    }

    /// Load a u64 value into this register (little-endian bit order).
    pub fn load_u64(&mut self, value: u64) {
        for i in 0..self.bits.len().min(64) {
            self.bits[i] = (value >> i) & 1 == 1;
        }
    }

    /// Read the register contents as a u64 (little-endian bit order).
    pub fn read_u64(&self) -> u64 {
        let mut value = 0u64;
        for (i, &bit) in self.bits.iter().enumerate() {
            if bit && i < 64 {
                value |= 1u64 << i;
            }
        }
        value
    }

    /// Number of qubits in this register.
    pub fn len(&self) -> usize {
        self.bits.len()
    }

    /// Check if register is empty.
    pub fn is_empty(&self) -> bool {
        self.bits.is_empty()
    }

    /// Check that all qubits are in the |0⟩ state (for ancilla verification).
    pub fn is_clean(&self) -> bool {
        self.bits.iter().all(|&b| !b)
    }

    /// Get a slice of qubit indices for a sub-range of this register.
    pub fn qubit_indices(&self, start: usize, len: usize) -> Vec<usize> {
        (start..start + len).map(|i| self.offset + i).collect()
    }
}
