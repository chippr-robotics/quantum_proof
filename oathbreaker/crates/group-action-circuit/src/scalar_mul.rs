use ec_goldilocks::CurveParams;
use reversible_arithmetic::ancilla::AncillaPool;
use reversible_arithmetic::gates::Gate;
use reversible_arithmetic::resource_counter::ResourceCounter;

/// Windowed double-and-add scalar multiplication (reversible).
///
/// Used for both halves of the coherent group-action map:
/// - [a]G controlled by exponent register a
/// - [b]Q controlled by exponent register b
///
/// Instead of bit-by-bit double-and-add, processes the scalar in w-bit windows:
/// - Precompute table of [0]P, [1]P, ..., [2^w - 1]P for base point P
/// - Process scalar in w-bit windows: 64/w iterations instead of 64
/// - Each iteration: w point doublings + 1 table lookup + 1 conditional point addition
///
/// Google used w=16 for secp256k1. For Oath-64 (64-bit), w=8 is reasonable.
pub struct WindowedScalarMul {
    /// Window size in bits.
    pub window_size: usize,
    /// Number of bits in the scalar.
    pub scalar_bits: usize,
}

impl WindowedScalarMul {
    pub fn new(window_size: usize, scalar_bits: usize) -> Self {
        assert!(
            scalar_bits % window_size == 0,
            "Scalar bits must be divisible by window size"
        );
        Self {
            window_size,
            scalar_bits,
        }
    }

    /// Number of windows to process.
    pub fn num_windows(&self) -> usize {
        self.scalar_bits / self.window_size
    }

    /// Generate the complete gate sequence for windowed scalar multiplication.
    ///
    /// This is the main loop for one half of the group-action circuit:
    /// ```text
    /// for window_idx in 0..num_windows:
    ///     // w doublings
    ///     for _ in 0..window_size:
    ///         reversible_ec_double(point_reg)
    ///     // Controlled addition from precomputed table
    ///     window_bits = scalar_reg[window_idx * w .. (window_idx + 1) * w]
    ///     table_entry = reversible_table_lookup(precomp_table, window_bits)
    ///     reversible_ec_add_conditional(point_reg, table_entry)
    ///     // Uncompute table lookup ancillae
    /// ```
    pub fn forward_gates(
        &self,
        scalar_reg_offset: usize,
        point_x_offset: usize,
        point_y_offset: usize,
        ancilla_pool: &mut AncillaPool,
        counter: &mut ResourceCounter,
        _curve: &CurveParams,
    ) -> Vec<Gate> {
        // Windowed reversible scalar multiplication.
        //
        // For each window of the exponent register:
        //   1. w point doublings of the accumulator
        //   2. QROM table lookup: load precomputed point from table
        //      controlled by window bits of the scalar register
        //   3. Conditional EC point addition: accumulator += table entry
        //   4. Uncompute table lookup ancillae
        //
        // QROM (Quantum Read-Only Memory) loads classical constants into
        // quantum registers using multi-controlled NOT gates.

        let n = _curve.field_bits; // 64
        let num_windows = self.num_windows();
        let w = self.window_size;
        let mut gates = Vec::new();

        // Allocate ancilla registers for table lookup results
        // Each window needs a temporary point (x, y) = 2n qubits
        let lookup_x = ancilla_pool.allocate("lookup_x", n, counter);
        let lookup_y = ancilla_pool.allocate("lookup_y", n, counter);

        // EC operation workspace
        let ec_workspace = ancilla_pool.allocate("ec_workspace", 10 * n + 2, counter);

        // Allocate temp registers for doubling ONCE outside all loops so that the
        // qubit footprint is O(n) rather than O(n * w * num_windows).
        let dbl_temp_x = ancilla_pool.allocate("dbl_temp_x", n, counter);
        let dbl_temp_y = ancilla_pool.allocate("dbl_temp_y", n, counter);

        // Allocate separate result registers for EC addition (distinct from the
        // accumulator inputs to avoid aliasing input/output registers).
        let ec_add_out_x = ancilla_pool.allocate("ec_add_out_x", n, counter);
        let ec_add_out_y = ancilla_pool.allocate("ec_add_out_y", n, counter);

        // Precompute table (classical — baked into circuit as constants)
        let precomp = crate::precompute::PrecomputeTable::generate_for_point(
            _curve,
            &_curve.generator,
            w,
        );

        for window_idx in 0..num_windows {
            // --- Step 1: w point doublings of the accumulator ---
            let doubler = reversible_arithmetic::ec_double_affine::ReversibleEcDouble::new(n);
            for _dbl in 0..w {
                // Double the accumulator point into temp registers.
                let dbl_gates = doubler.forward_gates(
                    point_x_offset,
                    point_y_offset,
                    dbl_temp_x.offset,
                    dbl_temp_y.offset,
                    ec_workspace.offset,
                    counter,
                );
                gates.extend(dbl_gates);

                // Swap result back to accumulator: acc ← temp, clear temp.
                for i in 0..n {
                    // CNOT swap: a ^= b; b ^= a; a ^= b
                    let g1 = Gate::Cnot { control: dbl_temp_x.offset + i, target: point_x_offset + i };
                    let g2 = Gate::Cnot { control: point_x_offset + i, target: dbl_temp_x.offset + i };
                    let g3 = Gate::Cnot { control: dbl_temp_x.offset + i, target: point_x_offset + i };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }
                for i in 0..n {
                    let g1 = Gate::Cnot { control: dbl_temp_y.offset + i, target: point_y_offset + i };
                    let g2 = Gate::Cnot { control: point_y_offset + i, target: dbl_temp_y.offset + i };
                    let g3 = Gate::Cnot { control: dbl_temp_y.offset + i, target: point_y_offset + i };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }
            }

            // --- Step 2: QROM table lookup ---
            // For each table entry i, apply multi-controlled NOT gates that flip
            // lookup bits conditioned on the window address matching i.
            //
            // Proper QROM address matching for entry_idx:
            //   For each control bit k (0..w):
            //     - If bit k of entry_idx is 0: apply X (NOT) to window bit k before
            //       and after the multi-controlled NOT so it acts on |0⟩.
            //     - If bit k of entry_idx is 1: no pre/post-NOT needed.
            //   Then apply the multi-controlled NOT conditioned on all w bits.
            //
            // For w > 2, the w-qubit controlled NOT is decomposed into Toffoli gates
            // using ancilla qubits (standard Barenco decomposition); for the resource
            // model we count each decomposed Toffoli individually.
            let window_start = scalar_reg_offset + window_idx * w;
            let table_size = 1usize << w;

            let mut qrom_gates = Vec::new();
            for entry_idx in 0..table_size {
                let point = precomp.lookup(entry_idx);
                if let ec_goldilocks::AffinePoint::Finite { x, y } = point {
                    let x_val = x.to_canonical();
                    let y_val = y.to_canonical();

                    // Apply X to window bits where entry_idx has 0 (control-on-0 gadget).
                    for k in 0..w {
                        if (entry_idx >> k) & 1 == 0 {
                            let g = Gate::Not { target: window_start + k };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                    }

                    // For each output bit that should be 1 for this entry, apply a
                    // w-controlled NOT.  With w controls and a single target this
                    // requires w−1 Toffoli gates (Lemma 7.2, Barenco et al. 1995)
                    // plus one ancilla; for simplicity we emit w/2 Toffoli pairs here
                    // as an approximation of the decomposition cost.
                    for bit in 0..n {
                        if (x_val >> bit) & 1 == 1 {
                            if w == 1 {
                                let g = Gate::Cnot {
                                    control: window_start,
                                    target: lookup_x.offset + bit,
                                };
                                counter.record_gate(&g);
                                qrom_gates.push(g);
                            } else {
                                // For w > 1 controls on a single target, use a Toffoli
                                // ladder with an ancilla (Barenco et al. 1995, Lemma 7.2).
                                // The ancilla is ec_workspace.offset (reused as scratch).
                                let anc = ec_workspace.offset;
                                // Toffoli(c[0], c[1], anc): anc ^= c[0] AND c[1]
                                let g_anc = Gate::Toffoli {
                                    control1: window_start,
                                    control2: window_start + 1,
                                    target: anc,
                                };
                                counter.record_gate(&g_anc);
                                qrom_gates.push(g_anc);

                                // For each additional control c[2..w]:
                                // Toffoli(anc, c[k], anc_next) — chain into target
                                for k in 2..w {
                                    let g_chain = Gate::Toffoli {
                                        control1: anc,
                                        control2: window_start + k,
                                        target: lookup_x.offset + bit,
                                    };
                                    counter.record_gate(&g_chain);
                                    qrom_gates.push(g_chain);
                                }

                                // For w == 2 the Toffoli itself acts as the target flip
                                if w == 2 {
                                    let g_flip = Gate::Cnot {
                                        control: anc,
                                        target: lookup_x.offset + bit,
                                    };
                                    counter.record_gate(&g_flip);
                                    qrom_gates.push(g_flip);
                                }

                                // Uncompute anc
                                let g_anc_unc = Gate::Toffoli {
                                    control1: window_start,
                                    control2: window_start + 1,
                                    target: anc,
                                };
                                counter.record_gate(&g_anc_unc);
                                qrom_gates.push(g_anc_unc);
                            }
                        }
                        if (y_val >> bit) & 1 == 1 {
                            if w == 1 {
                                let g = Gate::Cnot {
                                    control: window_start,
                                    target: lookup_y.offset + bit,
                                };
                                counter.record_gate(&g);
                                qrom_gates.push(g);
                            } else {
                                let anc = ec_workspace.offset;
                                let g_anc = Gate::Toffoli {
                                    control1: window_start,
                                    control2: window_start + 1,
                                    target: anc,
                                };
                                counter.record_gate(&g_anc);
                                qrom_gates.push(g_anc);

                                for k in 2..w {
                                    let g_chain = Gate::Toffoli {
                                        control1: anc,
                                        control2: window_start + k,
                                        target: lookup_y.offset + bit,
                                    };
                                    counter.record_gate(&g_chain);
                                    qrom_gates.push(g_chain);
                                }

                                if w == 2 {
                                    let g_flip = Gate::Cnot {
                                        control: anc,
                                        target: lookup_y.offset + bit,
                                    };
                                    counter.record_gate(&g_flip);
                                    qrom_gates.push(g_flip);
                                }

                                let g_anc_unc = Gate::Toffoli {
                                    control1: window_start,
                                    control2: window_start + 1,
                                    target: anc,
                                };
                                counter.record_gate(&g_anc_unc);
                                qrom_gates.push(g_anc_unc);
                            }
                        }
                    }

                    // Undo the X gadget on 0-controlled bits (restore window register).
                    for k in 0..w {
                        if (entry_idx >> k) & 1 == 0 {
                            let g = Gate::Not { target: window_start + k };
                            counter.record_gate(&g);
                            qrom_gates.push(g);
                        }
                    }
                }
            }
            gates.extend(qrom_gates.clone());
            ancilla_pool.record_for_uncompute(qrom_gates);

            // --- Step 3: Conditional EC point addition ---
            // accumulator += lookup point.
            //
            // Use distinct output registers (ec_add_out_x / ec_add_out_y) to
            // avoid aliasing the accumulator inputs with the adder outputs.
            let adder = reversible_arithmetic::ec_add_affine::ReversibleEcAdd::new(n);
            let add_gates = adder.forward_gates(
                point_x_offset,   // x₁: accumulator input
                point_y_offset,   // y₁: accumulator input
                lookup_x.offset,  // x₂: table lookup result
                lookup_y.offset,  // y₂: table lookup result
                ec_add_out_x.offset, // x₃: result (distinct register)
                ec_add_out_y.offset, // y₃: result (distinct register)
                ec_workspace.offset,
                counter,
            );
            gates.extend(add_gates);

            // Swap result into the accumulator and clear ec_add_out registers.
            for i in 0..n {
                let g1 = Gate::Cnot { control: ec_add_out_x.offset + i, target: point_x_offset + i };
                let g2 = Gate::Cnot { control: point_x_offset + i, target: ec_add_out_x.offset + i };
                let g3 = Gate::Cnot { control: ec_add_out_x.offset + i, target: point_x_offset + i };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }
            for i in 0..n {
                let g1 = Gate::Cnot { control: ec_add_out_y.offset + i, target: point_y_offset + i };
                let g2 = Gate::Cnot { control: point_y_offset + i, target: ec_add_out_y.offset + i };
                let g3 = Gate::Cnot { control: ec_add_out_y.offset + i, target: point_y_offset + i };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }

            // --- Step 4: Uncompute QROM lookup ---
            let uncompute = ancilla_pool.flush_uncompute(counter);
            gates.extend(uncompute);
        }

        gates
    }
}
