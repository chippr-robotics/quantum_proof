use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible elliptic curve point doubling.
///
/// Given P = (x₁, y₁), computes R = 2P = (x₃, y₃).
///
/// Uses the tangent slope: λ = (3x₁² + a) / (2y₁)
///
/// Shares most gate logic with point addition — the only difference
/// is the slope computation.
pub struct ReversibleEcDouble {
    /// Number of bits per field element.
    pub n: usize,
}

impl ReversibleEcDouble {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the full gate sequence for reversible point doubling.
    ///
    /// Register layout:
    /// - x1[0..n], y1[0..n]: input point
    /// - x3[0..n], y3[0..n]: result point (output)
    /// - workspace: ancilla qubits for intermediates
    ///
    /// Decomposition:
    /// 1. Compute x₁²                  (reversible squaring)
    /// 2. Compute 3x₁² + a             (reversible multiply-by-3 + add constant)
    /// 3. Compute 2y₁                   (reversible left-shift / doubling)
    /// 4. Compute (2y₁)⁻¹              (reversible inversion)
    /// 5. Compute λ = (3x₁² + a) · (2y₁)⁻¹  (reversible multiplication)
    /// 6. Compute x₃ = λ² - 2x₁        (reversible square + subtract)
    /// 7. Compute y₃ = λ(x₁ - x₃) - y₁ (reversible multiply + subtract)
    /// 8. Uncompute all intermediates
    pub fn forward_gates(
        &self,
        x1_offset: usize,
        y1_offset: usize,
        x3_offset: usize,
        y3_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // TODO: Implement reversible EC point doubling.
        todo!("Reversible EC point doubling")
    }
}
