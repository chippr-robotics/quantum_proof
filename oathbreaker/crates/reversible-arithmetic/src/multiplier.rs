use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible modular multiplier for GF(p).
///
/// Decomposes multiplication into controlled additions (schoolbook method):
/// |a⟩|b⟩|0⟩ → |a⟩|b⟩|a*b mod p⟩
///
/// Gate count: O(n²) Toffoli for n-bit operands.
/// Ancilla: accumulator register (n+1 bits) + reduction workspace.
pub struct ReversibleMultiplier {
    /// Number of bits per operand.
    pub n: usize,
}

impl ReversibleMultiplier {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular multiplication.
    ///
    /// Register layout:
    /// - a[0..n]: first operand (preserved)
    /// - b[0..n]: second operand (preserved)
    /// - result[0..n]: output register (starts at 0, ends with a*b mod p)
    /// - workspace: ancilla qubits for intermediate values
    ///
    /// The multiplication uses schoolbook decomposition:
    /// a * b = Σ_i a_i * b * 2^i
    /// Each bit a_i controls a conditional addition of (b << i) to the accumulator.
    ///
    /// After accumulation, reduce modulo p using the special form:
    /// p = 2^64 - 2^32 + 1, so 2^64 ≡ 2^32 - 1 (mod p).
    ///
    /// Intermediate values are uncomputed via Bennett's compute-copy-uncompute strategy.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement reversible schoolbook multiplication.
        //
        // Steps:
        // 1. For each bit i of a:
        //    a. Controlled on a[i], add (b << i) to accumulator
        //    b. This requires a controlled modular adder
        // 2. Reduce accumulator mod p (using Goldilocks reduction)
        // 3. Copy result to output register
        // 4. Uncompute accumulator
        todo!("Reversible modular multiplier")
    }
}

/// Reversible modular squaring, specialized for better gate count.
///
/// Since both inputs are the same, some gate optimizations are possible.
pub struct ReversibleSquarer {
    pub n: usize,
}

impl ReversibleSquarer {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement reversible squaring with optimizations.
        todo!("Reversible modular squarer")
    }
}
