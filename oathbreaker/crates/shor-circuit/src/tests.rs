#[cfg(test)]
mod tests {
    use crate::precompute::PrecomputeTable;
    use crate::qft::QuantumFourierTransform;

    #[test]
    fn test_qft_resource_count_64() {
        let qft = QuantumFourierTransform::new(64);
        let resources = qft.resource_count();

        assert_eq!(resources.hadamard_count, 64);
        assert_eq!(resources.controlled_rotation_count, 64 * 63 / 2);
        assert_eq!(resources.swap_count, 32);
    }

    #[test]
    fn test_qft_gate_sequence_small() {
        let qft = QuantumFourierTransform::new(4);
        let gates = qft.gate_sequence(0);

        // 4 Hadamards + 6 controlled rotations + 2 swaps = 12 gates
        assert_eq!(gates.len(), 12);
    }

    // TODO: Add tests that verify the full Shor circuit once
    // the reversible arithmetic implementations are complete.
    //
    // Test plan:
    // 1. Build circuit with real Sage-generated curve parameters
    // 2. Execute classically on random scalar k
    // 3. Verify output matches ec_goldilocks::scalar_mul(k, G)
    // 4. Cross-check with Pollard's rho on the same instance
}
