#[cfg(test)]
mod reversible_tests {
    use crate::gates::Gate;
    use crate::register::QuantumRegister;
    use crate::resource_counter::ResourceCounter;

    #[test]
    fn test_not_gate_is_self_inverse() {
        let mut bits = vec![false, true, false];
        let gate = Gate::Not { target: 1 };

        gate.apply(&mut bits);
        assert!(!bits[1]); // flipped

        gate.apply(&mut bits);
        assert!(bits[1]); // flipped back
    }

    #[test]
    fn test_cnot_gate() {
        // CNOT with control=0, target=1
        let gate = Gate::Cnot {
            control: 0,
            target: 1,
        };

        // Control is 0 → target unchanged
        let mut bits = vec![false, false];
        gate.apply(&mut bits);
        assert!(!bits[1]);

        // Control is 1 → target flipped
        let mut bits = vec![true, false];
        gate.apply(&mut bits);
        assert!(bits[1]);

        // Self-inverse
        gate.apply(&mut bits);
        assert!(!bits[1]);
    }

    #[test]
    fn test_toffoli_gate() {
        let gate = Gate::Toffoli {
            control1: 0,
            control2: 1,
            target: 2,
        };

        // Both controls 1 → target flipped
        let mut bits = vec![true, true, false];
        gate.apply(&mut bits);
        assert!(bits[2]);

        // One control 0 → target unchanged
        let mut bits = vec![true, false, false];
        gate.apply(&mut bits);
        assert!(!bits[2]);

        // Self-inverse
        let mut bits = vec![true, true, true];
        gate.apply(&mut bits);
        assert!(!bits[2]);
        gate.apply(&mut bits);
        assert!(bits[2]);
    }

    #[test]
    fn test_register_load_read() {
        let mut reg = QuantumRegister::new("test", 64);
        let value = 0xDEAD_BEEF_CAFE_BABEu64;
        reg.load_u64(value);
        assert_eq!(reg.read_u64(), value);
    }

    #[test]
    fn test_register_clean_check() {
        let reg = QuantumRegister::new("ancilla", 8);
        assert!(reg.is_clean());

        let mut reg2 = QuantumRegister::new("dirty", 8);
        reg2.load_u64(1);
        assert!(!reg2.is_clean());
    }

    #[test]
    fn test_resource_counter() {
        let mut counter = ResourceCounter::new();

        counter.record_gate(&Gate::Toffoli {
            control1: 0,
            control2: 1,
            target: 2,
        });
        counter.record_gate(&Gate::Cnot {
            control: 0,
            target: 1,
        });
        counter.record_gate(&Gate::Not { target: 0 });

        assert_eq!(counter.toffoli_count, 1);
        assert_eq!(counter.cnot_count, 1);
        assert_eq!(counter.not_count, 1);
        assert_eq!(counter.total_gates(), 3);
    }

    #[test]
    fn test_gate_inverse_is_self() {
        let gates = vec![
            Gate::Not { target: 0 },
            Gate::Cnot {
                control: 0,
                target: 1,
            },
            Gate::Toffoli {
                control1: 0,
                control2: 1,
                target: 2,
            },
        ];

        for gate in &gates {
            assert_eq!(
                gate.inverse(),
                *gate,
                "Gate should be self-inverse: {:?}",
                gate
            );
        }
    }
}
