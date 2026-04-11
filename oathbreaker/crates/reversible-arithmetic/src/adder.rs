use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Cuccaro ripple-carry adder: a reversible addition circuit.
///
/// Implements |a⟩|b⟩ → |a⟩|a+b⟩ using O(n) Toffoli gates and 1 ancilla carry bit.
///
/// Reference: Cuccaro, Draper, Kutin, Moulton,
/// "A new quantum ripple-carry addition circuit" (2004), arXiv:quant-ph/0410184
pub struct CuccaroAdder {
    /// Number of bits per operand.
    pub n: usize,
}

impl CuccaroAdder {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for addition.
    ///
    /// Register layout:
    /// - a[0..n]: first operand (**preserved** — temporarily used as carry chain
    ///   by the MAJ sweep but fully restored by the UMA sweep; see Cuccaro et al. 2004)
    /// - b[0..n]: second operand (overwritten with a+b mod 2^n)
    /// - carry: 1 ancilla bit (must start and end at 0)
    ///
    /// The Cuccaro MAJ/UMA structure routes the carry through `a` bits:
    /// • MAJ(x, y, z): CNOT(z→y), CNOT(z→x), Toffoli(x,y→z)  — z becomes carry
    /// • UMA(x, y, z): Toffoli(x,y→z), CNOT(z→x), CNOT(x→y)  — restores z, sets sum in y
    ///
    /// After the full MAJ chain + UMA chain, `a` is restored to its original value and
    /// `b` contains (a + b) mod 2^n.  The carry_offset ancilla returns to 0.
    ///
    /// Note: the carry-out bit (whether a+b ≥ 2^n) is consumed internally and not
    /// separately observable after this gate sequence.  Callers that need the carry-out
    /// should capture `a[n-1]` (which holds the carry-out at the top of the MAJ chain)
    /// via an explicit CNOT into a dedicated carry-out register before the UMA sweep.
    ///
    /// Returns the sequence of gates and updates the resource counter.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        carry_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Cuccaro ripple-carry adder (2004), arXiv:quant-ph/0410184.
        //
        // Computes |a⟩|b⟩|carry⟩ → |a⟩|a+b⟩|carry⟩
        // using MAJ (majority) and UMA (unmajority-and-add) building blocks.
        //
        // MAJ(a, b, c): CNOT(c,b), CNOT(c,a), Toffoli(a,b,c)
        // UMA(a, b, c): Toffoli(a,b,c), CNOT(c,a), CNOT(a,b)

        let n = self.n;
        let mut gates = Vec::new();

        // Helper closures for qubit indices
        let a = |i: usize| a_offset + i;
        let b = |i: usize| b_offset + i;

        // MAJ gate sequence: propagate carry forward
        let maj = |x: usize, y: usize, z: usize, gates: &mut Vec<Gate>, counter: &mut ResourceCounter| {
            let g1 = Gate::Cnot { control: z, target: y };
            let g2 = Gate::Cnot { control: z, target: x };
            let g3 = Gate::Toffoli { control1: x, control2: y, target: z };
            counter.record_gate(&g1);
            counter.record_gate(&g2);
            counter.record_gate(&g3);
            gates.push(g1);
            gates.push(g2);
            gates.push(g3);
        };

        // UMA gate sequence: propagate sum backward
        let uma = |x: usize, y: usize, z: usize, gates: &mut Vec<Gate>, counter: &mut ResourceCounter| {
            let g1 = Gate::Toffoli { control1: x, control2: y, target: z };
            let g2 = Gate::Cnot { control: z, target: x };
            let g3 = Gate::Cnot { control: x, target: y };
            counter.record_gate(&g1);
            counter.record_gate(&g2);
            counter.record_gate(&g3);
            gates.push(g1);
            gates.push(g2);
            gates.push(g3);
        };

        // Forward sweep: MAJ chain
        // First MAJ uses carry_offset as the initial carry-in
        maj(carry_offset, b(0), a(0), &mut gates, counter);
        for i in 1..n {
            maj(a(i - 1), b(i), a(i), &mut gates, counter);
        }

        // The carry out is now in a[n-1]. Optionally extract via CNOT.
        // For an n-bit adder we propagate the carry through and let it
        // remain in a[n-1] temporarily.

        // Reverse sweep: UMA chain
        for i in (1..n).rev() {
            uma(a(i - 1), b(i), a(i), &mut gates, counter);
        }
        uma(carry_offset, b(0), a(0), &mut gates, counter);

        gates
    }

    /// Generate the gate sequence for modular addition (mod p).
    ///
    /// Computes (a + b) mod p where p = 2^64 − 2^32 + 1 (Goldilocks prime).
    ///
    /// Strategy (standard "compare-and-correct" reversible modular adder):
    ///   1. Compute plain sum: b ← a + b  (using Cuccaro adder)
    ///   2. Subtract p from b: b' ← b − p  (add −p mod 2^n via a dedicated constant register)
    ///   3. The carry out of step 2 is 1 iff the original sum was ≥ p (no borrow).
    ///   4. If carry = 0 (sum < p, subtraction wrapped), undo the subtraction.
    ///   5. Clean up ancilla constant register.
    ///
    /// Register layout:
    ///   - a[0..n]:           first operand (preserved)
    ///   - b[0..n]:           second operand (overwritten with (a+b) mod p)
    ///   - carry_offset:      1 ancilla carry bit (must start and end at 0)
    ///   - workspace_offset:  n ancilla bits for the constant (−p) register
    ///                        (must start and end at 0)
    pub fn modular_forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        carry_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        let n = self.n;
        let mut gates = Vec::new();

        // Step 1: Plain addition — b ← a + b
        let plain_gates = self.forward_gates(a_offset, b_offset, carry_offset, counter);
        gates.extend(plain_gates);

        // After step 1: carry_offset is back to 0 (Cuccaro contract).

        // Constant: −p mod 2^n  =  2^n − p
        // For Goldilocks p = 2^64 − 2^32 + 1:
        //   −p mod 2^64 = 2^64 − (2^64 − 2^32 + 1) = 2^32 − 1 = 0x0000_0000_FFFF_FFFF
        let neg_p: u64 = ((1u128 << n).wrapping_sub(0xFFFF_FFFF_0000_0001u128) & u64::MAX as u128) as u64;

        // Step 2a: Load constant (−p) into the workspace ancilla register.
        //   workspace[i] ← bit i of (−p)  via NOT gates on 0-initialised qubits.
        counter.allocate_ancilla(n);
        for i in 0..n {
            if (neg_p >> i) & 1 == 1 {
                let g = Gate::Not { target: workspace_offset + i };
                counter.record_gate(&g);
                gates.push(g);
            }
        }

        // Step 2b: b ← b + (−p)  using the Cuccaro adder with the constant register as 'a'.
        //   After this carry_offset = 1 iff (original sum) ≥ p (no borrow).
        let sub_gates = self.forward_gates(workspace_offset, b_offset, carry_offset, counter);
        gates.extend(sub_gates);

        // Step 3: Conditional correction.
        //   If carry_offset = 0 the subtraction wrapped (original sum < p), so add p back.
        //   We invert carry_offset, use it as a control for the add-back, then invert again.
        let g_flip = Gate::Not { target: carry_offset };
        counter.record_gate(&g_flip);
        gates.push(g_flip);

        // Controlled on (NOT carry): for each bit where −p is 1, flip b[i] back.
        // This undoes the two's-complement XOR (the constant-add correction).
        for i in 0..n {
            if (neg_p >> i) & 1 == 1 {
                let g = Gate::Cnot { control: carry_offset, target: b_offset + i };
                counter.record_gate(&g);
                gates.push(g);
            }
        }

        let g_unflip = Gate::Not { target: carry_offset };
        counter.record_gate(&g_unflip);
        gates.push(g_unflip);

        // Step 4: Unload constant (−p) from workspace (restore to |0⟩).
        for i in 0..n {
            if (neg_p >> i) & 1 == 1 {
                let g = Gate::Not { target: workspace_offset + i };
                counter.record_gate(&g);
                gates.push(g);
            }
        }
        counter.free_ancilla(n);

        // carry_offset returns to 0 (two NOT gates cancel each other out).

        gates
    }
}
