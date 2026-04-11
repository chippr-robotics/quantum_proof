#[cfg(test)]
mod tests {
    use crate::qft_stub::QftResourceEstimate;

    #[test]
    fn test_qft_estimate_single_register_64() {
        let est = QftResourceEstimate::for_single_register(64);
        assert_eq!(est.hadamard_count, 64);
        assert_eq!(est.controlled_rotation_count, 64 * 63 / 2);
        assert_eq!(est.swap_count, 32);
        assert_eq!(est.num_registers, 1);
    }

    #[test]
    fn test_qft_estimate_dual_register_64() {
        let est = QftResourceEstimate::for_dual_register(64);
        // Dual register = 2× single register
        assert_eq!(est.hadamard_count, 128);
        assert_eq!(est.controlled_rotation_count, 64 * 63); // 2 * (64*63/2)
        assert_eq!(est.swap_count, 64);
        assert_eq!(est.num_registers, 2);
    }

    #[test]
    fn test_qft_estimate_small() {
        let est = QftResourceEstimate::for_single_register(4);
        assert_eq!(est.hadamard_count, 4);
        assert_eq!(est.controlled_rotation_count, 6); // 4*3/2
        assert_eq!(est.swap_count, 2);
        assert_eq!(est.total_gates, 12);
    }

    // TODO: Add integration tests that verify the full group-action circuit once
    // the reversible arithmetic implementations are complete.
    //
    // Test plan:
    // 1. Build circuit with real Sage-generated Oath-64 curve parameters
    // 2. For random (a, b, k), set Q = [k]G
    // 3. Execute circuit classically: result = [a]G + [b]Q
    // 4. Verify result matches ec_goldilocks::double_scalar_mul(a, G, b, Q)
    // 5. Cross-check: verify [a + k*b]G == result
}
