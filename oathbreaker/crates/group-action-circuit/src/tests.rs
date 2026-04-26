#[cfg(test)]
mod circuit_tests {
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
}

#[cfg(test)]
mod qft_tests {
    use crate::qft::Qft;
    use crate::quantum_gate::QuantumGate;

    #[test]
    fn test_qft_gate_count_matches_estimate() {
        // Verify generated gate counts match the resource estimate model
        // for several register sizes.
        for n in [4, 8, 16, 32, 64] {
            assert!(
                Qft::validate_against_estimate(n),
                "QFT gate count mismatch for n={}",
                n
            );
        }
    }

    #[test]
    fn test_qft_forward_gate_sequence_4() {
        let gates = Qft::forward_gates(0, 4);

        // Expected: H(0), CR2(1,0), CR3(2,0), CR4(3,0),
        //           H(1), CR2(2,1), CR3(3,1),
        //           H(2), CR2(3,2),
        //           H(3),
        //           SWAP(0,3), SWAP(1,2)
        let hadamards: Vec<_> = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Hadamard { .. }))
            .collect();
        assert_eq!(hadamards.len(), 4);

        let rotations: Vec<_> = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::ControlledPhase { .. }))
            .collect();
        assert_eq!(rotations.len(), 6); // 4*3/2

        let swaps: Vec<_> = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Swap { .. }))
            .collect();
        assert_eq!(swaps.len(), 2); // 4/2

        // Verify first gate is H(0)
        assert!(matches!(gates[0], QuantumGate::Hadamard { target: 0 }));

        // Verify first rotation is CR2(control=1, target=0)
        assert!(matches!(
            gates[1],
            QuantumGate::ControlledPhase {
                control: 1,
                target: 0,
                k: 2,
                sign: 1
            }
        ));
    }

    #[test]
    fn test_qft_inverse_negates_phases() {
        let forward = Qft::forward_gates(0, 4);
        let inverse = Qft::inverse_gates(0, 4);

        // Count gates — both should have same number of each type
        let count = |gates: &[QuantumGate], pred: fn(&QuantumGate) -> bool| -> usize {
            gates.iter().filter(|g| pred(g)).count()
        };

        let is_h = |g: &QuantumGate| matches!(g, QuantumGate::Hadamard { .. });
        let is_cr = |g: &QuantumGate| matches!(g, QuantumGate::ControlledPhase { .. });
        let is_swap = |g: &QuantumGate| matches!(g, QuantumGate::Swap { .. });

        assert_eq!(count(&forward, is_h), count(&inverse, is_h));
        assert_eq!(count(&forward, is_cr), count(&inverse, is_cr));
        assert_eq!(count(&forward, is_swap), count(&inverse, is_swap));

        // Verify inverse has sign = -1 on all controlled phases
        for gate in &inverse {
            if let QuantumGate::ControlledPhase { sign, .. } = gate {
                assert_eq!(*sign, -1, "Inverse QFT should have sign=-1");
            }
        }

        // Verify forward has sign = +1 on all controlled phases
        for gate in &forward {
            if let QuantumGate::ControlledPhase { sign, .. } = gate {
                assert_eq!(*sign, 1, "Forward QFT should have sign=+1");
            }
        }
    }

    #[test]
    fn test_qft_with_offset() {
        // QFT on register starting at qubit 64 (simulating reg_b)
        let gates = Qft::forward_gates(64, 4);

        // All qubit indices should be in [64, 68)
        for gate in &gates {
            for &q in &gate.qubits() {
                assert!((64..68).contains(&q), "Qubit {} out of offset range", q);
            }
        }
    }

    #[test]
    fn test_shor_qft_and_measure_dual_register() {
        let n = 8;
        let gates = Qft::shor_qft_and_measure(n);

        // Should have gates for: inverse QFT on reg_a + inverse QFT on reg_b + measurements
        let hadamards = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Hadamard { .. }))
            .count();
        let measurements = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Measure { .. }))
            .count();

        assert_eq!(hadamards, 2 * n); // n per register, 2 registers
        assert_eq!(measurements, 2 * n); // measure all qubits in both registers
    }

    #[test]
    fn test_measurement_gates_indices() {
        let gates = Qft::measurement_gates(10, 0, 4);
        assert_eq!(gates.len(), 4);

        for (i, gate) in gates.iter().enumerate() {
            match gate {
                QuantumGate::Measure {
                    qubit,
                    classical_bit,
                } => {
                    assert_eq!(*qubit, 10 + i);
                    assert_eq!(*classical_bit, i);
                }
                _ => panic!("Expected Measure gate"),
            }
        }
    }
}

#[cfg(test)]
mod qft_classical_sim_tests {
    use crate::qft::classical_sim::*;

    #[test]
    fn test_qft_basis_state_0() {
        // QFT|0⟩ = (1/√N) Σ_y |y⟩  — uniform superposition
        let n = 3;
        let size = 1usize << n;
        let mut state = vec![Complex::ZERO; size];
        state[0] = Complex::ONE;

        let result = apply_qft_direct(&state, n);

        let expected_amp = 1.0 / (size as f64).sqrt();
        for (y, r) in result.iter().enumerate() {
            assert!(
                (r.re - expected_amp).abs() < 1e-10,
                "QFT|0⟩ amplitude wrong at y={}: got {}, expected {}",
                y,
                r.re,
                expected_amp
            );
            assert!(
                r.im.abs() < 1e-10,
                "QFT|0⟩ should have zero imaginary part at y={}",
                y
            );
        }
    }

    #[test]
    fn test_qft_inverse_recovers_state() {
        // QFT† ∘ QFT = Identity
        let n = 3;
        let size = 1usize << n;

        // Start with |5⟩
        let mut state = vec![Complex::ZERO; size];
        state[5] = Complex::ONE;

        let after_qft = apply_qft_direct(&state, n);
        let recovered = apply_inverse_qft_direct(&after_qft, n);

        // Should recover |5⟩
        for (y, r) in recovered.iter().enumerate() {
            let expected = if y == 5 { 1.0 } else { 0.0 };
            assert!(
                (r.re - expected).abs() < 1e-10 && r.im.abs() < 1e-10,
                "Round-trip failed at y={}: got ({}, {}), expected {}",
                y,
                r.re,
                r.im,
                expected
            );
        }
    }

    #[test]
    fn test_qft_gates_match_direct() {
        // Verify the gate-by-gate simulation matches the direct DFT
        let n = 3;
        let size = 1usize << n;

        // Test with |3⟩
        let mut state_direct = vec![Complex::ZERO; size];
        state_direct[3] = Complex::ONE;

        let mut state_gates = state_direct.clone();

        let result_direct = apply_qft_direct(&state_direct, n);
        apply_qft_gates(&mut state_gates, n);

        for y in 0..size {
            assert!(
                (state_gates[y].re - result_direct[y].re).abs() < 1e-10
                    && (state_gates[y].im - result_direct[y].im).abs() < 1e-10,
                "Gate sim vs direct mismatch at y={}: gate=({:.6}, {:.6}), direct=({:.6}, {:.6})",
                y,
                state_gates[y].re,
                state_gates[y].im,
                result_direct[y].re,
                result_direct[y].im,
            );
        }
    }

    #[test]
    fn test_inverse_qft_gates_match_direct() {
        let n = 3;
        let size = 1usize << n;

        // Start with a QFT'd state, then apply inverse via gates and direct
        let mut initial = vec![Complex::ZERO; size];
        initial[6] = Complex::ONE;
        let qft_state = apply_qft_direct(&initial, n);

        let mut state_gates = qft_state.clone();
        apply_inverse_qft_gates(&mut state_gates, n);

        let result_direct = apply_inverse_qft_direct(&qft_state, n);

        for y in 0..size {
            assert!(
                (state_gates[y].re - result_direct[y].re).abs() < 1e-10
                    && (state_gates[y].im - result_direct[y].im).abs() < 1e-10,
                "Inverse gate vs direct mismatch at y={}",
                y,
            );
        }
    }

    #[test]
    fn test_qft_unitarity() {
        // The QFT should preserve the norm of the state vector
        let n = 3;
        let size = 1usize << n;

        let mut state = vec![Complex::ZERO; size];
        state[2] = Complex { re: 0.6, im: 0.0 };
        state[5] = Complex { re: 0.0, im: 0.8 };

        let norm_before: f64 = state.iter().map(|c| c.norm_sq()).sum();
        let result = apply_qft_direct(&state, n);
        let norm_after: f64 = result.iter().map(|c| c.norm_sq()).sum();

        assert!(
            (norm_before - norm_after).abs() < 1e-10,
            "QFT changed norm: {} → {}",
            norm_before,
            norm_after
        );
    }

    #[test]
    fn test_qft_4_qubit_gates_match_direct() {
        // Test a larger case to exercise more CR gates
        let n = 4;
        let size = 1usize << n;

        let mut state = vec![Complex::ZERO; size];
        state[7] = Complex::ONE;

        let mut state_gates = state.clone();
        let result_direct = apply_qft_direct(&state, n);
        apply_qft_gates(&mut state_gates, n);

        for y in 0..size {
            assert!(
                (state_gates[y].re - result_direct[y].re).abs() < 1e-10
                    && (state_gates[y].im - result_direct[y].im).abs() < 1e-10,
                "4-qubit gate/direct mismatch at y={}",
                y,
            );
        }
    }
}

#[cfg(test)]
mod continued_fraction_tests {
    use crate::continued_fraction::*;

    #[test]
    fn test_mod_inverse_basic() {
        assert_eq!(mod_inverse(3, 11), Some(4));
        assert_eq!(mod_inverse(7, 11), Some(8));
        assert_eq!(mod_inverse(2, 4), None); // gcd(2,4) = 2
        assert_eq!(mod_inverse(1, 97), Some(1));
    }

    #[test]
    fn test_mod_inverse_large() {
        let p = 65537u64;
        let a = 12345u64;
        let inv = mod_inverse(a, p).unwrap();
        assert_eq!((a as u128 * inv as u128) % p as u128, 1);
    }

    #[test]
    fn test_recover_secret_direct_prime_order() {
        let order = 251u64; // Prime
        for k in [0, 1, 42, 100, 250] {
            let d = 100u64;
            let c =
                ((order as u128 - (k as u128 * d as u128) % order as u128) % order as u128) as u64;
            let recovered = recover_secret_direct(c, d, order);
            assert_eq!(recovered, Some(k), "Failed to recover k={}", k);
        }
    }

    #[test]
    fn test_recover_secret_various_d() {
        let order = 997u64;
        let k = 42u64;

        for d in [1, 2, 5, 13, 100, 500, 996] {
            let c =
                ((order as u128 - (k as u128 * d as u128) % order as u128) % order as u128) as u64;
            let recovered = recover_secret_direct(c, d, order);
            assert_eq!(recovered, Some(k), "Failed for d={}", d);
        }
    }

    #[test]
    fn test_recover_secret_multi_all_valid() {
        let order = 251u64;
        let k = 42u64;

        let pairs: Vec<(u64, u64)> = (1..=5u64)
            .map(|d| {
                let c = ((order as u128 - (k as u128 * d as u128) % order as u128) % order as u128)
                    as u64;
                (c, d)
            })
            .collect();

        assert_eq!(recover_secret_multi(&pairs, order), Some(k));
    }

    #[test]
    fn test_continued_fraction_7_over_3() {
        let convergents = continued_fraction_convergents(7, 3);
        assert_eq!(convergents, vec![(2, 1), (7, 3)]);
    }

    #[test]
    fn test_continued_fraction_355_over_113() {
        let convergents = continued_fraction_convergents(355, 113);
        let last = convergents.last().unwrap();
        assert_eq!(*last, (355, 113));
    }

    #[test]
    fn test_continued_fraction_integer() {
        let convergents = continued_fraction_convergents(10, 1);
        assert_eq!(convergents, vec![(10, 1)]);
    }

    #[test]
    fn test_continued_fraction_fibonacci_ratio() {
        // 89/55 (Fibonacci ratio) → convergents approach golden ratio
        let convergents = continued_fraction_convergents(89, 55);
        let last = convergents.last().unwrap();
        assert_eq!(*last, (89, 55));
        // All intermediate convergents should be Fibonacci ratios
        assert!(convergents.len() >= 2);
    }

    #[test]
    fn test_gcd_basic() {
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(17, 13), 1);
        assert_eq!(gcd(100, 25), 25);
        assert_eq!(gcd(0, 5), 5);
        assert_eq!(gcd(7, 0), 7);
        assert_eq!(gcd(1, 1), 1);
    }
}

#[cfg(test)]
mod quantum_gate_tests {
    use crate::quantum_gate::{QuantumGate, QuantumGateCount};
    use reversible_arithmetic::gates::Gate;

    #[test]
    fn test_quantum_gate_qasm_reversible() {
        let not_gate = QuantumGate::Reversible(Gate::Not { target: 0 });
        assert_eq!(not_gate.to_qasm(), "x q[0];");

        let cnot = QuantumGate::Reversible(Gate::Cnot {
            control: 0,
            target: 1,
        });
        assert_eq!(cnot.to_qasm(), "cx q[0], q[1];");

        let toffoli = QuantumGate::Reversible(Gate::Toffoli {
            control1: 0,
            control2: 1,
            target: 2,
        });
        assert_eq!(toffoli.to_qasm(), "ccx q[0], q[1], q[2];");
    }

    #[test]
    fn test_quantum_gate_qasm_qft() {
        let h = QuantumGate::Hadamard { target: 5 };
        assert_eq!(h.to_qasm(), "h q[5];");

        let cr = QuantumGate::ControlledPhase {
            control: 1,
            target: 0,
            k: 3,
            sign: 1,
        };
        assert_eq!(cr.to_qasm(), "cp(2*pi/2**3) q[1], q[0];");

        let cr_inv = QuantumGate::ControlledPhase {
            control: 1,
            target: 0,
            k: 3,
            sign: -1,
        };
        assert_eq!(cr_inv.to_qasm(), "cp(-2*pi/2**3) q[1], q[0];");

        let swap = QuantumGate::Swap {
            qubit_a: 0,
            qubit_b: 7,
        };
        assert_eq!(swap.to_qasm(), "swap q[0], q[7];");

        let measure = QuantumGate::Measure {
            qubit: 3,
            classical_bit: 3,
        };
        assert_eq!(measure.to_qasm(), "c[3] = measure q[3];");
    }

    #[test]
    fn test_quantum_gate_count() {
        use crate::qft::Qft;

        let gates = Qft::shor_qft_and_measure(8);
        let counts = QuantumGateCount::from_gates(&gates);

        assert_eq!(counts.hadamard, 16); // 8 * 2 registers
        assert_eq!(counts.controlled_phase, 8 * 7); // 2 * 8*7/2
        assert_eq!(counts.swap, 8); // 2 * 8/2
        assert_eq!(counts.measurement, 16); // 2 * 8
        assert_eq!(counts.toffoli, 0); // No Toffoli in QFT
    }

    #[test]
    fn test_quantum_gate_qubits() {
        let h = QuantumGate::Hadamard { target: 5 };
        assert_eq!(h.qubits(), vec![5]);

        let cr = QuantumGate::ControlledPhase {
            control: 3,
            target: 1,
            k: 2,
            sign: 1,
        };
        assert_eq!(cr.qubits(), vec![3, 1]);

        let swap = QuantumGate::Swap {
            qubit_a: 0,
            qubit_b: 7,
        };
        assert_eq!(swap.qubits(), vec![0, 7]);
    }
}

#[cfg(test)]
mod shor_end_to_end_tests {
    use crate::shor::ShorsEcdlp;
    use ec_oath::curve::{AffinePoint, CurveParams};
    use ec_oath::point_ops::scalar_mul;
    use oath_field::GoldilocksField;

    /// Create a small test curve for end-to-end Shor's testing.
    ///
    /// Uses a curve with known small order so that ECDLP tests run fast.
    fn test_curve_small() -> CurveParams {
        let a = GoldilocksField::new(1);
        let b = GoldilocksField::new(3);

        let gx = GoldilocksField::new(0);
        let gy_sq = gx * gx * gx + a * gx + b;
        let gy = gy_sq.sqrt().expect("y² = 3 should be a QR");

        let generator = AffinePoint::Finite { x: gx, y: gy };

        // Use a small field_bits for fast circuit construction.
        // Order is a placeholder — we'll use brute force to find
        // a valid k for testing.
        CurveParams {
            a,
            b,
            order: GoldilocksField::P,
            generator,
            field_bits: 8, // Small for fast tests
            prime_modulus: GoldilocksField::P,
        }
    }

    #[test]
    fn test_shors_build_circuit() {
        let curve = test_curve_small();
        let shor = ShorsEcdlp::build(&curve, 4);

        // Verify QFT gates are generated
        assert!(!shor.qft_measurement_gates.is_empty());

        // Verify gate counts are populated
        assert!(shor.gate_counts.hadamard > 0);
        assert!(shor.gate_counts.controlled_phase > 0);
        assert!(shor.gate_counts.swap > 0);
        assert!(shor.gate_counts.measurement > 0);
        assert!(shor.gate_counts.toffoli > 0); // From group-action circuit

        // For n=8: dual register QFT
        assert_eq!(shor.gate_counts.hadamard, 16); // 2 * 8
        assert_eq!(shor.gate_counts.measurement, 16); // 2 * 8
    }

    #[test]
    fn test_shors_classical_verification() {
        let curve = test_curve_small();

        // Pick a known secret k
        let k = 42u64;
        let target_q = scalar_mul(k, &curve.generator, &curve);

        let shor = ShorsEcdlp::build(&curve, 4);
        let result = shor.run_classical_verification(&target_q, k, 10);

        assert!(result.verified, "Shor's should recover and verify k");
        assert_eq!(result.recovered_k, Some(k));
        assert!(result.direct_recovery_count > 0);
        assert_eq!(result.num_trials, 10);
        assert_eq!(result.field_bits, 8);
    }

    #[test]
    fn test_shors_various_secrets() {
        let curve = test_curve_small();

        for k in [1u64, 2, 7, 42, 100, 255] {
            let target_q = scalar_mul(k, &curve.generator, &curve);
            let shor = ShorsEcdlp::build(&curve, 4);
            let result = shor.run_classical_verification(&target_q, k, 5);

            assert!(
                result.verified,
                "Failed to verify for k={}. Recovered: {:?}",
                k, result.recovered_k
            );
            assert_eq!(result.recovered_k, Some(k), "Wrong k recovered for k={}", k);
        }
    }

    #[test]
    fn test_shors_zero_secret() {
        let curve = test_curve_small();
        let k = 0u64;
        let target_q = AffinePoint::Infinity; // [0]G = O

        let shor = ShorsEcdlp::build(&curve, 4);
        let result = shor.run_classical_verification(&target_q, k, 5);

        assert!(result.verified);
        assert_eq!(result.recovered_k, Some(0));
    }

    #[test]
    fn test_shors_summary_format() {
        let curve = test_curve_small();
        let shor = ShorsEcdlp::build(&curve, 4);
        let summary = shor.summary();

        // Verify summary contains key sections
        assert!(summary.contains("Stage 1"));
        assert!(summary.contains("Stage 2"));
        assert!(summary.contains("Stage 3"));
        assert!(summary.contains("Hadamard"));
        assert!(summary.contains("jacobian"));
    }

    #[test]
    fn test_shors_gate_count_consistency() {
        let curve = test_curve_small();
        let shor = ShorsEcdlp::build(&curve, 4);
        let n = curve.field_bits;

        // QFT gate counts should match formula
        let expected_hadamards = 2 * n; // n per register, 2 registers
        let expected_rotations = 2 * (n * (n - 1) / 2);
        let expected_swaps = 2 * (n / 2);
        let expected_measurements = 2 * n;

        assert_eq!(shor.gate_counts.hadamard, expected_hadamards);
        assert_eq!(shor.gate_counts.controlled_phase, expected_rotations);
        assert_eq!(shor.gate_counts.swap, expected_swaps);
        assert_eq!(shor.gate_counts.measurement, expected_measurements);
    }
}

#[cfg(test)]
mod export_tests {
    use crate::export::{export_qasm, export_shor_qasm, export_stats_json};
    use crate::shor::ShorsEcdlp;
    use ec_oath::curve::{AffinePoint, CurveParams};
    use oath_field::GoldilocksField;

    fn test_curve_small() -> CurveParams {
        let a = GoldilocksField::new(1);
        let b = GoldilocksField::new(3);
        let gx = GoldilocksField::new(0);
        let gy = (gx * gx * gx + a * gx + b).sqrt().unwrap();

        CurveParams {
            a,
            b,
            order: GoldilocksField::P,
            generator: AffinePoint::Finite { x: gx, y: gy },
            field_bits: 8,
            prime_modulus: GoldilocksField::P,
        }
    }

    #[test]
    fn test_export_group_action_qasm() {
        let curve = test_curve_small();
        let circuit = crate::build_group_action_circuit_jacobian(&curve, 4);
        let qasm = export_qasm(&circuit);

        assert!(qasm.starts_with("OPENQASM 3.0;"));
        assert!(qasm.contains("include \"stdgates.inc\""));
        assert!(qasm.contains("qubit["));
        assert!(qasm.contains("QFT gates:"));
    }

    #[test]
    fn test_export_shor_qasm() {
        let curve = test_curve_small();
        let shor = ShorsEcdlp::build(&curve, 4);
        let qasm = export_shor_qasm(&shor);

        assert!(qasm.starts_with("OPENQASM 3.0;"));
        assert!(qasm.contains("Stage 1: Group-action map"));
        assert!(qasm.contains("Stage 2: Inverse QFT"));
        assert!(qasm.contains("Stage 3: Measurement"));
        assert!(qasm.contains("bit[")); // Classical register
        assert!(qasm.contains("h q[")); // Hadamard gates
        assert!(qasm.contains("cp(")); // Controlled phase
        assert!(qasm.contains("swap q[")); // SWAP gates
        assert!(qasm.contains("measure q[")); // Measurements
    }

    #[test]
    fn test_export_stats_json() {
        let curve = test_curve_small();
        let circuit = crate::build_group_action_circuit_jacobian(&curve, 4);
        let json = export_stats_json(&circuit);

        assert!(json.contains("\"field_bits\""));
        assert!(json.contains("\"toffoli_gates\""));
        assert!(json.contains("\"qft_hadamards_estimated\""));
    }
}

#[cfg(test)]
mod group_action_integration_tests {
    use crate::double_scalar::build_group_action_circuit_jacobian;
    use ec_oath::curve::{AffinePoint, CurveParams};
    use ec_oath::point_ops::scalar_mul;
    use oath_field::GoldilocksField;

    fn test_curve() -> CurveParams {
        let a = GoldilocksField::new(1);
        let b = GoldilocksField::new(3);
        let gx = GoldilocksField::new(0);
        let gy = (gx * gx * gx + a * gx + b).sqrt().unwrap();

        CurveParams {
            a,
            b,
            order: GoldilocksField::P,
            generator: AffinePoint::Finite { x: gx, y: gy },
            field_bits: 8,
            prime_modulus: GoldilocksField::P,
        }
    }

    #[test]
    fn test_group_action_circuit_classical_correctness() {
        // Implements the TODO from the original tests.rs:
        // Build circuit, classically verify [a]G + [b]Q matches reference.
        let curve = test_curve();
        let circuit = build_group_action_circuit_jacobian(&curve, 4);

        let k = 42u64;
        let target_q = scalar_mul(k, &curve.generator, &curve);

        // Test several (a, b) pairs
        let test_cases: Vec<(u64, u64)> =
            vec![(0, 0), (1, 0), (0, 1), (1, 1), (3, 5), (7, 11), (100, 200)];

        for (a, b) in test_cases {
            let circuit_result = circuit.execute_classical(a, b, &target_q);
            let reference = ec_oath::double_scalar_mul(a, &curve.generator, b, &target_q, &curve);
            assert_eq!(circuit_result, reference, "Mismatch for a={}, b={}", a, b);
        }
    }

    #[test]
    fn test_group_action_linearity() {
        // For Q = [k]G: [a]G + [b]Q = [a]G + [bk]G = [a + bk]G
        let curve = test_curve();
        let circuit = build_group_action_circuit_jacobian(&curve, 4);

        let k = 42u64;
        let target_q = scalar_mul(k, &curve.generator, &curve);

        for (a, b) in [(1, 1), (3, 5), (7, 11)] {
            let result = circuit.execute_classical(a, b, &target_q);
            // [a + b*k]G should equal [a]G + [b]Q
            let combined_scalar =
                ((a as u128 + b as u128 * k as u128) % curve.order as u128) as u64;
            let expected = scalar_mul(combined_scalar, &curve.generator, &curve);
            assert_eq!(
                result, expected,
                "Linearity failed: [{}]G + [{}]Q ≠ [{}]G",
                a, b, combined_scalar
            );
        }
    }
}
