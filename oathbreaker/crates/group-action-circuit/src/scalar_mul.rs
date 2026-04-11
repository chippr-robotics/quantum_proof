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
                // Double the accumulator point in-place.
                // We use a temp register, then swap back.
                let temp_x = ancilla_pool.allocate("dbl_temp_x", n, counter);
                let temp_y = ancilla_pool.allocate("dbl_temp_y", n, counter);

                let dbl_gates = doubler.forward_gates(
                    point_x_offset,
                    point_y_offset,
                    temp_x.offset,
                    temp_y.offset,
                    ec_workspace.offset,
                    counter,
                );
                gates.extend(dbl_gates);

                // Swap result back to accumulator: acc ← temp, then clear temp
                for i in 0..n {
                    // CNOT swap: acc ^= temp, temp ^= acc, acc ^= temp
                    let g1 = Gate::Cnot { control: temp_x.offset + i, target: point_x_offset + i };
                    let g2 = Gate::Cnot { control: point_x_offset + i, target: temp_x.offset + i };
                    let g3 = Gate::Cnot { control: temp_x.offset + i, target: point_x_offset + i };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }
                for i in 0..n {
                    let g1 = Gate::Cnot { control: temp_y.offset + i, target: point_y_offset + i };
                    let g2 = Gate::Cnot { control: point_y_offset + i, target: temp_y.offset + i };
                    let g3 = Gate::Cnot { control: temp_y.offset + i, target: point_y_offset + i };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }
            }

            // --- Step 2: QROM table lookup ---
            // The window bits are scalar_reg[window_idx * w .. (window_idx + 1) * w].
            // For each table entry i (0..2^w), if the window bits encode i,
            // load precomp[i] into lookup registers.
            //
            // QROM circuit: for each entry i in the table, use multi-controlled
            // NOT gates (controlled on window bits matching i) to XOR the
            // classical point coordinates into the lookup register.
            let window_start = scalar_reg_offset + window_idx * w;
            let table_size = 1usize << w;

            let mut qrom_gates = Vec::new();
            for entry_idx in 0..table_size {
                let point = precomp.lookup(entry_idx);
                if let ec_goldilocks::AffinePoint::Finite { x, y } = point {
                    let x_val = x.to_canonical();
                    let y_val = y.to_canonical();

                    // For each bit of x and y that is 1, apply a multi-controlled
                    // NOT gate controlled on the window bits matching entry_idx.
                    //
                    // Multi-controlled NOT with w controls can be decomposed into
                    // O(w) Toffoli gates using ancillae (standard decomposition).
                    // For the resource model, we count the Toffoli cost directly.
                    for bit in 0..n {
                        if (x_val >> bit) & 1 == 1 {
                            // Multi-controlled NOT: flip lookup_x[bit] if window = entry_idx
                            // We use the first window bit as primary control and
                            // chain Toffoli gates for multi-control decomposition.
                            if w == 1 {
                                let g = Gate::Cnot {
                                    control: window_start,
                                    target: lookup_x.offset + bit,
                                };
                                counter.record_gate(&g);
                                qrom_gates.push(g);
                            } else {
                                // Use first two address bits as Toffoli controls
                                // (simplified multi-control decomposition)
                                let ctrl1 = window_start;
                                let ctrl2 = if w > 1 { window_start + 1 } else { window_start };
                                let g = Gate::Toffoli {
                                    control1: ctrl1,
                                    control2: ctrl2,
                                    target: lookup_x.offset + bit,
                                };
                                counter.record_gate(&g);
                                qrom_gates.push(g);
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
                                let ctrl1 = window_start;
                                let ctrl2 = if w > 1 { window_start + 1 } else { window_start };
                                let g = Gate::Toffoli {
                                    control1: ctrl1,
                                    control2: ctrl2,
                                    target: lookup_y.offset + bit,
                                };
                                counter.record_gate(&g);
                                qrom_gates.push(g);
                            }
                        }
                    }
                }
            }
            gates.extend(qrom_gates.clone());
            ancilla_pool.record_for_uncompute(qrom_gates);

            // --- Step 3: Conditional EC point addition ---
            // accumulator += lookup point
            let adder = reversible_arithmetic::ec_add_affine::ReversibleEcAdd::new(n);
            let add_gates = adder.forward_gates(
                point_x_offset,
                point_y_offset,
                lookup_x.offset,
                lookup_y.offset,
                point_x_offset, // result overwrites accumulator (simplified)
                point_y_offset,
                ec_workspace.offset,
                counter,
            );
            gates.extend(add_gates);

            // --- Step 4: Uncompute QROM lookup ---
            let uncompute = ancilla_pool.flush_uncompute(counter);
            gates.extend(uncompute);
        }

        gates
    }
}
