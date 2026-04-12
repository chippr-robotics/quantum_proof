use ec_goldilocks::CurveParams;
use reversible_arithmetic::ancilla::AncillaPool;
use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::resource_counter::ResourceCounter;

/// Windowed scalar multiplication in Jacobian projective coordinates.
///
/// This is the optimized replacement for the affine-coordinate scalar
/// multiplication. The accumulator stays in Jacobian (X, Y, Z) throughout,
/// using:
/// - **Jacobian point doubling** (no inversions) for the w doublings per window
/// - **Jacobian mixed addition** (no inversions) to add affine precomputed
///   table entries to the Jacobian accumulator
///
/// The result is a ~6× reduction in Toffoli gate count compared to affine,
/// because per-addition field inversion (~96 multiplication-equivalents) is
/// eliminated entirely. Only one inversion is needed at the very end to
/// convert the Jacobian accumulator back to affine.
///
/// QROM table lookup uses a one-hot binary decode of all w scalar bits to
/// select the correct precomputed point from a 2^w-entry table. Cost:
/// O(2^w) Toffoli for decode/undecode + O(2^w * n) CNOT for table loading.
///
/// Register allocation:
///   Affine:   point_x(n) + point_y(n) = 2n qubits
///   Jacobian: point_X(n) + point_Y(n) + point_Z(n) = 3n qubits
///   Cost: +n qubits + 2^w qubits (one-hot register).
///   Savings: eliminate ~(num_windows * 2 - 1) inversions.
pub struct WindowedScalarMulJacobian {
    /// Window size in bits.
    pub window_size: usize,
    /// Number of bits in the scalar.
    pub scalar_bits: usize,
}

impl WindowedScalarMulJacobian {
    pub fn new(window_size: usize, scalar_bits: usize) -> Self {
        assert!(
            scalar_bits.is_multiple_of(window_size),
            "Scalar bits must be divisible by window size"
        );
        Self {
            window_size,
            scalar_bits,
        }
    }

    pub fn num_windows(&self) -> usize {
        self.scalar_bits / self.window_size
    }

    /// Generate the complete gate sequence for windowed scalar multiplication
    /// using Jacobian projective coordinates.
    ///
    /// The accumulator is in Jacobian (X, Y, Z). Precomputed table entries
    /// are in affine (x, y). Each window iteration uses:
    /// 1. w Jacobian doublings (0 inversions each)
    /// 2. 1 QROM table lookup (affine point loaded into ancilla)
    /// 3. 1 Jacobian mixed addition (0 inversions)
    /// 4. Uncompute QROM ancillae
    #[allow(clippy::too_many_arguments)]
    pub fn forward_gates(
        &self,
        scalar_reg_offset: usize,
        point_x_offset: usize, // Jacobian X
        point_y_offset: usize, // Jacobian Y
        point_z_offset: usize, // Jacobian Z
        ancilla_pool: &mut AncillaPool,
        counter: &mut ResourceCounter,
        curve: &CurveParams,
    ) -> Vec<Gate> {
        let n = curve.field_bits;
        let num_windows = self.num_windows();
        let w = self.window_size;
        let mut gates = Vec::new();

        // Allocate ancilla registers for table lookup results (affine point)
        let lookup_x = ancilla_pool.allocate("lookup_x", n, counter);
        let lookup_y = ancilla_pool.allocate("lookup_y", n, counter);

        // Temp registers for doubling/addition output
        let temp_x = ancilla_pool.allocate("jac_temp_x", n, counter);
        let temp_y = ancilla_pool.allocate("jac_temp_y", n, counter);
        let temp_z = ancilla_pool.allocate("jac_temp_z", n, counter);

        // EC operation workspace — Jacobian ops need more workspace than affine
        // Mixed add: ~13n, Doubling: ~12n. Take the max + multiplier workspace.
        let ec_workspace = ancilla_pool.allocate("ec_workspace_jac", 13 * n + 1, counter);

        // One-hot selection register for QROM decode (2^w qubits).
        // Decodes the w-bit window index into a one-hot representation so that
        // exactly one bit is set, selecting the correct precomputed table entry.
        let table_size = 1usize << w;
        let one_hot = ancilla_pool.allocate("qrom_one_hot", table_size, counter);

        // Precompute table (classical — baked into circuit as constants)
        let precomp =
            crate::precompute::PrecomputeTable::generate_for_point(curve, &curve.generator, w);

        let doubler = reversible_arithmetic::ec_double_jacobian::ReversibleJacobianDouble::new(n);
        let adder = reversible_arithmetic::ec_add_jacobian::ReversibleJacobianMixedAdd::new(n);

        for window_idx in 0..num_windows {
            // --- Step 1: w Jacobian doublings of the accumulator ---
            for _dbl in 0..w {
                let dbl_gates = doubler.forward_gates(
                    point_x_offset,
                    point_y_offset,
                    point_z_offset,
                    temp_x.offset,
                    temp_y.offset,
                    temp_z.offset,
                    ec_workspace.offset,
                    counter,
                );
                gates.extend(dbl_gates);

                // Swap result back to accumulator via CNOT swap
                for reg_pair in [
                    (temp_x.offset, point_x_offset),
                    (temp_y.offset, point_y_offset),
                    (temp_z.offset, point_z_offset),
                ] {
                    for i in 0..n {
                        let g1 = Gate::Cnot {
                            control: reg_pair.0 + i,
                            target: reg_pair.1 + i,
                        };
                        let g2 = Gate::Cnot {
                            control: reg_pair.1 + i,
                            target: reg_pair.0 + i,
                        };
                        let g3 = Gate::Cnot {
                            control: reg_pair.0 + i,
                            target: reg_pair.1 + i,
                        };
                        counter.record_gate(&g1);
                        counter.record_gate(&g2);
                        counter.record_gate(&g3);
                        gates.push(g1);
                        gates.push(g2);
                        gates.push(g3);
                    }
                }
            }

            // --- Step 2: QROM table lookup (one-hot binary decode) ---
            //
            // Decodes all w scalar bits into a 2^w one-hot selection register,
            // then uses each one-hot bit to CNOT-load the corresponding table
            // entry into the lookup registers. Finally, uncomputes the one-hot
            // register to return it to |0...0⟩.
            //
            // Decode cost: O(2^w) Toffoli + O(2^w) CNOT
            // Load cost:   O(2^w * n) CNOT (data-dependent)
            // Total QROM:  O(2^w) Toffoli + O(2^w * n) CNOT (×2 for undecode)
            let window_start = scalar_reg_offset + window_idx * w;

            let mut qrom_gates = Vec::new();

            // --- 2a: One-hot binary decode ---
            // Convert w scalar bits at window_start into a one-hot register.
            //
            // Algorithm: start with one_hot[0] = 1. For each scalar bit k,
            // split each active entry into two: the original (bit k = 0) and
            // a new one (bit k = 1), using Toffoli + CNOT pairs.
            //
            // After processing all w bits, exactly one_hot[idx] = 1 where
            // idx is the integer value of the w scalar bits.

            // Initialize: set one_hot[0] = 1
            let g_init = Gate::Not {
                target: one_hot.offset,
            };
            counter.record_gate(&g_init);
            qrom_gates.push(g_init);

            // Process each scalar bit to build the one-hot encoding
            for k in 0..w {
                let stride = 1usize << k;
                let block = 1usize << (k + 1);

                // Phase A: For each index j with bit k SET (descending order),
                // set one_hot[j] = one_hot[j - stride] AND s[k].
                // Descending order ensures we don't read from a target we just wrote.
                let mut j = block.min(table_size);
                while j > 0 {
                    j -= 1;
                    if j & stride != 0 && j < table_size && j >= stride {
                        let g = Gate::Toffoli {
                            control1: one_hot.offset + j - stride,
                            control2: window_start + k,
                            target: one_hot.offset + j,
                        };
                        counter.record_gate(&g);
                        qrom_gates.push(g);
                    }
                }

                // Phase B: For each index j with bit k CLEAR (ascending order),
                // update one_hot[j] = one_hot[j] AND NOT(s[k]) by XOR-ing with
                // the partner: one_hot[j] ^= one_hot[j + stride].
                for j in 0..block.min(table_size) {
                    if j & stride == 0 {
                        let partner = j + stride;
                        if partner < table_size {
                            let g = Gate::Cnot {
                                control: one_hot.offset + partner,
                                target: one_hot.offset + j,
                            };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                    }
                }
            }

            // --- 2b: Table load using one-hot selection ---
            // For each table entry, if one_hot[entry_idx] = 1, CNOT the entry's
            // coordinate bits into the lookup registers. Since exactly one
            // one_hot bit is set, exactly one entry gets loaded.
            for entry_idx in 0..table_size {
                let point = precomp.lookup(entry_idx);
                if let ec_goldilocks::AffinePoint::Finite { x, y } = point {
                    let x_val = x.to_canonical();
                    let y_val = y.to_canonical();

                    for bit in 0..n {
                        if (x_val >> bit) & 1 == 1 {
                            let g = Gate::Cnot {
                                control: one_hot.offset + entry_idx,
                                target: lookup_x.offset + bit,
                            };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                        if (y_val >> bit) & 1 == 1 {
                            let g = Gate::Cnot {
                                control: one_hot.offset + entry_idx,
                                target: lookup_y.offset + bit,
                            };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                    }
                }
            }

            // --- 2c: One-hot undecode (reverse of decode) ---
            // Run the decode gates in reverse order to return one_hot to |0...0⟩.
            // Since all gates are self-inverse, this cleanly uncomputes the
            // one-hot register while preserving the loaded lookup values.
            for k in (0..w).rev() {
                let stride = 1usize << k;
                let block = 1usize << (k + 1);

                // Reverse of Phase B: CNOT in descending order
                let mut j = block.min(table_size);
                while j > 0 {
                    j -= 1;
                    if j & stride == 0 {
                        let partner = j + stride;
                        if partner < table_size {
                            let g = Gate::Cnot {
                                control: one_hot.offset + partner,
                                target: one_hot.offset + j,
                            };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                    }
                }

                // Reverse of Phase A: Toffoli in ascending order
                for j in 0..block.min(table_size) {
                    if j & stride != 0 && j >= stride {
                        let g = Gate::Toffoli {
                            control1: one_hot.offset + j - stride,
                            control2: window_start + k,
                            target: one_hot.offset + j,
                        };
                        counter.record_gate(&g);
                        qrom_gates.push(g);
                    }
                }
            }

            // Undo initialization: one_hot[0] back to 0
            let g_uninit = Gate::Not {
                target: one_hot.offset,
            };
            counter.record_gate(&g_uninit);
            qrom_gates.push(g_uninit);

            gates.extend(qrom_gates.clone());
            ancilla_pool.record_for_uncompute(qrom_gates);

            // --- Step 3: Jacobian mixed addition ---
            // accumulator (Jacobian) += lookup point (affine)
            let add_gates = adder.forward_gates(
                point_x_offset,
                point_y_offset,
                point_z_offset,
                lookup_x.offset,
                lookup_y.offset,
                temp_x.offset,
                temp_y.offset,
                temp_z.offset,
                ec_workspace.offset,
                counter,
            );
            gates.extend(add_gates);

            // Swap result back to accumulator
            for reg_pair in [
                (temp_x.offset, point_x_offset),
                (temp_y.offset, point_y_offset),
                (temp_z.offset, point_z_offset),
            ] {
                for i in 0..n {
                    let g1 = Gate::Cnot {
                        control: reg_pair.0 + i,
                        target: reg_pair.1 + i,
                    };
                    let g2 = Gate::Cnot {
                        control: reg_pair.1 + i,
                        target: reg_pair.0 + i,
                    };
                    let g3 = Gate::Cnot {
                        control: reg_pair.0 + i,
                        target: reg_pair.1 + i,
                    };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }
            }

            // --- Step 4: Uncompute QROM lookup ---
            // Replays the QROM gates in reverse to clean lookup registers.
            // This re-decodes the one-hot, undoes the table load, then
            // uncomputes the one-hot again.
            let uncompute = ancilla_pool.flush_uncompute(counter);
            gates.extend(uncompute);
        }

        gates
    }
}
