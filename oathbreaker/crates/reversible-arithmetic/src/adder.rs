use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Cuccaro ripple-carry adder: a reversible addition circuit.
///
/// Implements |a⟩|b⟩ → |a⟩|a+b⟩ using O(n) Toffoli gates and 1 ancilla carry bit.
///
/// Reference: Cuccaro, Draper, Kutin, Moulton,
/// "A new quantum ripple-carry addition circuit" (2004), arXiv:quant-ph/0410184
pub struct CuccaroAdder {
    /// Number of bits per operand.
    pub n: usize,
}

impl CuccaroAdder {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for addition.
    ///
    /// Register layout:
    /// - a[0..n]: first operand (preserved)
    /// - b[0..n]: second operand (overwritten with a+b)
    /// - carry: 1 ancilla bit (must start and end at 0)
    ///
    /// Returns the sequence of gates and updates the resource counter.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        carry_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement the full Cuccaro adder gate sequence.
        //
        // The Cuccaro adder uses a "majority" and "unmajority-and-add" (UMA) structure:
        //
        // MAJ(a, b, c):
        //   CNOT(c, b)
        //   CNOT(c, a)
        //   Toffoli(a, b, c)
        //
        // UMA(a, b, c):
        //   Toffoli(a, b, c)
        //   CNOT(c, a)
        //   CNOT(a, b)
        //
        // The full adder chains MAJ gates forward and UMA gates backward.
        todo!("Cuccaro ripple-carry adder gate sequence")
    }

    /// Generate the gate sequence for modular addition (mod p).
    ///
    /// After plain addition, conditionally subtract p if result >= p.
    pub fn modular_forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        carry_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement modular addition.
        //
        // Strategy:
        // 1. Compute a + b (plain addition)
        // 2. Compute a + b - p (subtraction)
        // 3. If borrow: keep a + b, else keep a + b - p
        // 4. Uncompute the intermediate values
        todo!("Reversible modular adder")
    }
}
