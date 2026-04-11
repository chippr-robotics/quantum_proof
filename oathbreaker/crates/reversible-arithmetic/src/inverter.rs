use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible modular inversion via Fermat's little theorem.
///
/// Computes a^(p-2) mod p via reversible square-and-multiply.
/// For p = 2^64 - 2^32 + 1:
///   p - 2 = 0xFFFF_FFFE_FFFF_FFFF
///   Hamming weight ≈ 63, so ~63 multiplications + 63 squarings.
///
/// Each intermediate squaring must be uncomputed to free ancilla qubits.
pub struct FermatInverter {
    /// Number of bits in the field.
    pub n: usize,
}

impl FermatInverter {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular inversion via Fermat.
    ///
    /// This is the most expensive single operation in the Shor circuit.
    /// Each step requires a reversible multiplication plus uncomputation.
    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement reversible Fermat inversion.
        //
        // The exponent p - 2 = 2^64 - 2^32 - 1 has a nice addition chain:
        //
        // Strategy: compute a^(p-2) using an efficient addition chain for
        // the exponent, minimizing the number of multiplications.
        //
        // Each multiplication step:
        // 1. Compute product in workspace (reversible multiply)
        // 2. Copy result to output
        // 3. Uncompute workspace (run multiply in reverse)
        //
        // This is expensive but conceptually straightforward.
        // Can be optimized later with binary GCD (Kaliski) method.
        todo!("Reversible Fermat inversion")
    }
}

/// Reversible modular inversion via binary GCD (Kaliski method).
///
/// Lower gate count than Fermat but more complex control flow.
/// Requires many conditional swaps and shifts.
///
/// Reference: Roetteler et al., "Quantum Resource Estimates for Computing
/// Elliptic Curve Discrete Logarithms" (2017)
pub struct BinaryGcdInverter {
    pub n: usize,
}

impl BinaryGcdInverter {
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
        // TODO: Implement binary GCD inversion (optimization target).
        // This is a stretch goal — Fermat is sufficient for the initial version.
        todo!("Reversible binary GCD inversion")
    }
}
