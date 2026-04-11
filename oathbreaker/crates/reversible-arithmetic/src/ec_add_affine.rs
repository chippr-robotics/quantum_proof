use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible elliptic curve point addition.
///
/// Given P = (x₁, y₁) and Q = (x₂, y₂), computes R = P + Q = (x₃, y₃).
///
/// This is the subroutine that Google proved for secp256k1. Our version
/// operates on 64-bit registers over the Goldilocks field.
///
/// Decomposition:
/// 1. Compute Δx = x₂ - x₁         (reversible subtraction)
/// 2. Compute Δy = y₂ - y₁         (reversible subtraction)
/// 3. Compute Δx⁻¹                  (reversible inversion — most expensive)
/// 4. Compute λ = Δy · Δx⁻¹        (reversible multiplication)
/// 5. Compute λ²                    (reversible squaring)
/// 6. Compute x₃ = λ² - x₁ - x₂   (reversible subtraction)
/// 7. Compute y₃ = λ(x₁ - x₃) - y₁ (reversible multiply + subtract)
/// 8. Uncompute all intermediates    (λ, Δx, Δy, Δx⁻¹, λ²)
pub struct ReversibleEcAdd {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleEcAdd {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the full gate sequence for reversible point addition.
    ///
    /// Register layout:
    /// - x1[0..n], y1[0..n]: first point (may be modified depending on variant)
    /// - x2[0..n], y2[0..n]: second point (may be modified)
    /// - x3[0..n], y3[0..n]: result point (output)
    /// - workspace: ancilla qubits for intermediates (Δx, Δy, Δx⁻¹, λ, λ²)
    ///
    /// The workspace requires approximately 5n qubits for intermediates,
    /// plus whatever the inversion subroutine needs internally.
    pub fn forward_gates(
        &self,
        x1_offset: usize,
        y1_offset: usize,
        x2_offset: usize,
        y2_offset: usize,
        x3_offset: usize,
        y3_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement reversible EC point addition.
        //
        // This is the most important subroutine in the entire project.
        // The gate sequence must:
        // - Compute the slope λ = (y₂ - y₁) / (x₂ - x₁)
        // - Compute the result point coordinates
        // - Uncompute ALL intermediate values to return ancillae to |0⟩
        //
        // The uncomputation doubles the gate count but is essential for
        // maintaining the quantum circuit's reversibility invariant.
        todo!("Reversible EC point addition")
    }

    /// Estimated resource cost for one point addition.
    pub fn estimated_resources(&self) -> (usize, usize) {
        // Rough estimates for n-bit field:
        // - 1 inversion: ~64 multiplications (Fermat) = ~64 * n² Toffoli
        // - 3 multiplications: ~3 * n² Toffoli
        // - Several additions/subtractions: ~O(n) Toffoli each
        // - Uncomputation: roughly doubles the above
        //
        // Qubits: ~12n (2 input points + 1 output point + ~6n workspace)
        let qubits = 12 * self.n;
        let toffoli = 2 * (64 + 3) * self.n * self.n; // rough estimate
        (qubits, toffoli)
    }
}
