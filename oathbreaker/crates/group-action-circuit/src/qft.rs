use crate::quantum_gate::QuantumGate;
use crate::qft_stub::QftResourceEstimate;

/// Quantum Fourier Transform circuit generator.
///
/// The QFT maps computational basis states to the Fourier basis:
///   QFT|x⟩ = (1/√N) Σ_y exp(2πixy/N) |y⟩,  where N = 2^n.
///
/// For Shor's ECDLP algorithm, the **inverse QFT** is applied to both
/// exponent registers after the coherent group-action map. This converts
/// the phase information encoded by the group homomorphism into
/// computational basis states that can be measured.
///
/// Gate decomposition (standard textbook QFT):
///   for j = 0 to n-1:
///       H(q[j])
///       for m = 2 to n-j:
///           CR_m(control=q[j+m-1], target=q[j])
///   for j = 0 to ⌊n/2⌋-1:
///       SWAP(q[j], q[n-1-j])
///
/// Gate counts per register:
///   Hadamard:           n
///   Controlled-Phase:   n(n-1)/2
///   SWAP:               ⌊n/2⌋
pub struct Qft;

impl Qft {
    /// Generate the forward QFT gate sequence for an n-qubit register.
    ///
    /// The register occupies qubits [offset .. offset+n).
    pub fn forward_gates(offset: usize, n: usize) -> Vec<QuantumGate> {
        let mut gates = Vec::new();

        for j in 0..n {
            // Hadamard on qubit j
            gates.push(QuantumGate::Hadamard {
                target: offset + j,
            });

            // Controlled phase rotations: CR_m for m = 2, 3, ..., n-j
            for m in 2..=(n - j) {
                gates.push(QuantumGate::ControlledPhase {
                    control: offset + j + m - 1,
                    target: offset + j,
                    k: m,
                    sign: 1,
                });
            }
        }

        // Bit-reversal via SWAPs
        for j in 0..n / 2 {
            gates.push(QuantumGate::Swap {
                qubit_a: offset + j,
                qubit_b: offset + n - 1 - j,
            });
        }

        gates
    }

    /// Generate the inverse QFT (QFT†) gate sequence for an n-qubit register.
    ///
    /// The inverse QFT reverses the gate order and conjugates all phases
    /// (sign → -sign on controlled rotations). Hadamard and SWAP are
    /// self-adjoint and remain unchanged.
    ///
    /// This is the transform applied in Shor's algorithm before measurement.
    pub fn inverse_gates(offset: usize, n: usize) -> Vec<QuantumGate> {
        let mut gates = Vec::new();

        // Bit-reversal first (reversed order from forward QFT)
        for j in (0..n / 2).rev() {
            gates.push(QuantumGate::Swap {
                qubit_a: offset + j,
                qubit_b: offset + n - 1 - j,
            });
        }

        // Reverse the H + CR sequence
        for j in (0..n).rev() {
            // Controlled phase rotations in reverse order with negated phase
            for m in (2..=(n - j)).rev() {
                gates.push(QuantumGate::ControlledPhase {
                    control: offset + j + m - 1,
                    target: offset + j,
                    k: m,
                    sign: -1, // Conjugated phase for inverse
                });
            }

            // Hadamard (self-adjoint)
            gates.push(QuantumGate::Hadamard {
                target: offset + j,
            });
        }

        gates
    }

    /// Generate measurement gates for an n-qubit register.
    ///
    /// Maps qubit [offset+i] to classical bit [classical_offset+i].
    pub fn measurement_gates(
        qubit_offset: usize,
        classical_offset: usize,
        n: usize,
    ) -> Vec<QuantumGate> {
        (0..n)
            .map(|i| QuantumGate::Measure {
                qubit: qubit_offset + i,
                classical_bit: classical_offset + i,
            })
            .collect()
    }

    /// Generate the complete QFT + measurement sequence for Shor's dual-register
    /// ECDLP formulation.
    ///
    /// Applies inverse QFT independently to both exponent registers, then
    /// measures both registers in the computational basis.
    ///
    /// Register layout (matches `double_scalar.rs`):
    ///   reg_a: qubits [0 .. n)         — first exponent register
    ///   reg_b: qubits [n .. 2n)        — second exponent register
    pub fn shor_qft_and_measure(n: usize) -> Vec<QuantumGate> {
        let mut gates = Vec::new();

        // Inverse QFT on register a (qubits 0..n)
        gates.extend(Self::inverse_gates(0, n));

        // Inverse QFT on register b (qubits n..2n)
        gates.extend(Self::inverse_gates(n, n));

        // Measure register a → classical bits 0..n
        gates.extend(Self::measurement_gates(0, 0, n));

        // Measure register b → classical bits n..2n
        gates.extend(Self::measurement_gates(n, n, n));

        gates
    }

    /// Validate gate counts against the resource estimate model.
    ///
    /// Returns true if the generated gate sequence matches the expected
    /// counts from `QftResourceEstimate`.
    pub fn validate_against_estimate(n: usize) -> bool {
        let gates = Self::forward_gates(0, n);
        let estimate = QftResourceEstimate::for_single_register(n);

        let hadamards = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Hadamard { .. }))
            .count();
        let rotations = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::ControlledPhase { .. }))
            .count();
        let swaps = gates
            .iter()
            .filter(|g| matches!(g, QuantumGate::Swap { .. }))
            .count();

        hadamards == estimate.hadamard_count
            && rotations == estimate.controlled_rotation_count
            && swaps == estimate.swap_count
    }
}

/// Classical simulation of the QFT for small register sizes (testing only).
///
/// Operates on a state vector of 2^n complex amplitudes. Only feasible
/// for n ≤ 20 due to exponential memory. Used to verify gate correctness
/// against the direct DFT matrix.
pub mod classical_sim {
    use std::f64::consts::PI;

    /// A complex number for state vector simulation.
    #[derive(Clone, Copy, Debug)]
    pub struct Complex {
        pub re: f64,
        pub im: f64,
    }

    impl Complex {
        pub const ZERO: Self = Self { re: 0.0, im: 0.0 };
        pub const ONE: Self = Self { re: 1.0, im: 0.0 };

        pub fn from_polar(r: f64, theta: f64) -> Self {
            Self {
                re: r * theta.cos(),
                im: r * theta.sin(),
            }
        }

        pub fn norm_sq(&self) -> f64 {
            self.re * self.re + self.im * self.im
        }

        pub fn conj(&self) -> Self {
            Self {
                re: self.re,
                im: -self.im,
            }
        }
    }

    impl std::ops::Mul for Complex {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            Self {
                re: self.re * rhs.re - self.im * rhs.im,
                im: self.re * rhs.im + self.im * rhs.re,
            }
        }
    }

    impl std::ops::Add for Complex {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            Self {
                re: self.re + rhs.re,
                im: self.im + rhs.im,
            }
        }
    }

    impl std::ops::Sub for Complex {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            Self {
                re: self.re - rhs.re,
                im: self.im - rhs.im,
            }
        }
    }

    impl std::ops::Mul<Complex> for f64 {
        type Output = Complex;
        fn mul(self, rhs: Complex) -> Complex {
            Complex {
                re: self * rhs.re,
                im: self * rhs.im,
            }
        }
    }

    /// Apply the QFT directly via the DFT matrix (O(N²) reference implementation).
    ///
    /// QFT|x⟩ = (1/√N) Σ_y ω^{xy} |y⟩  where ω = e^{2πi/N}.
    pub fn apply_qft_direct(state: &[Complex], n: usize) -> Vec<Complex> {
        let size = 1usize << n;
        assert_eq!(state.len(), size);

        let norm = 1.0 / (size as f64).sqrt();
        let mut result = vec![Complex::ZERO; size];

        for (y, res) in result.iter_mut().enumerate() {
            for (x, &s) in state.iter().enumerate() {
                let angle = 2.0 * PI * (x as f64) * (y as f64) / (size as f64);
                let omega = Complex::from_polar(1.0, angle);
                *res = *res + s * omega;
            }
            *res = norm * *res;
        }

        result
    }

    /// Apply the inverse QFT directly via the conjugate DFT matrix.
    pub fn apply_inverse_qft_direct(state: &[Complex], n: usize) -> Vec<Complex> {
        let size = 1usize << n;
        assert_eq!(state.len(), size);

        let norm = 1.0 / (size as f64).sqrt();
        let mut result = vec![Complex::ZERO; size];

        for (y, res) in result.iter_mut().enumerate() {
            for (x, &s) in state.iter().enumerate() {
                let angle = -2.0 * PI * (x as f64) * (y as f64) / (size as f64);
                let omega = Complex::from_polar(1.0, angle);
                *res = *res + s * omega;
            }
            *res = norm * *res;
        }

        result
    }

    /// Apply the QFT gate-by-gate to a state vector (verifies gate decomposition).
    ///
    /// This simulates each H, CR, and SWAP gate individually on the full
    /// 2^n state vector. The result should match `apply_qft_direct`.
    ///
    /// Note: The state vector uses LSB-first encoding (qubit 0 = bit 0 of
    /// the index). The textbook QFT circuit assumes qubit 0 = MSB. To get
    /// the standard DFT, we apply gates in MSB-to-LSB order: qubit n-1
    /// first (the MSB in LSB-first encoding).
    pub fn apply_qft_gates(state: &mut [Complex], n: usize) {
        let size = 1usize << n;
        assert_eq!(state.len(), size);

        // Apply in MSB-first order for LSB-first state vector encoding.
        // Textbook "qubit j" maps to simulation "qubit n-1-j".
        for j in (0..n).rev() {
            apply_hadamard(state, j, n);

            for k in (0..j).rev() {
                let m = j - k + 1; // Rotation denominator
                apply_controlled_phase(state, k, j, m, 1, n);
            }
        }

        // Bit-reversal SWAPs
        for j in 0..n / 2 {
            apply_swap(state, j, n - 1 - j, n);
        }
    }

    /// Apply the inverse QFT gate-by-gate to a state vector.
    pub fn apply_inverse_qft_gates(state: &mut [Complex], n: usize) {
        let size = 1usize << n;
        assert_eq!(state.len(), size);

        // Reverse bit-reversal SWAPs
        for j in (0..n / 2).rev() {
            apply_swap(state, j, n - 1 - j, n);
        }

        // Reverse the gate sequence with conjugated phases (MSB-first order reversed)
        for j in 0..n {
            for k in 0..j {
                let m = j - k + 1;
                apply_controlled_phase(state, k, j, m, -1, n);
            }
            apply_hadamard(state, j, n);
        }
    }

    /// Apply Hadamard gate to qubit `target` in an n-qubit state vector.
    fn apply_hadamard(state: &mut [Complex], target: usize, n: usize) {
        let size = 1usize << n;
        let inv_sqrt2 = 1.0 / 2.0_f64.sqrt();
        let bit = 1usize << target;

        let mut i = 0;
        while i < size {
            // Process pairs where qubit `target` is 0 vs 1
            if i & bit == 0 {
                let j = i | bit;
                let a = state[i];
                let b = state[j];
                state[i] = inv_sqrt2 * (a + b);
                state[j] = inv_sqrt2 * (a - b);
            }
            i += 1;
        }
    }

    /// Apply controlled phase rotation CR_k to the state vector.
    ///
    /// When both `control` and `target` qubits are |1⟩, applies
    /// phase e^{sign * 2πi/2^k}.
    fn apply_controlled_phase(
        state: &mut [Complex],
        control: usize,
        target: usize,
        k: usize,
        sign: i8,
        n: usize,
    ) {
        let size = 1usize << n;
        let angle = sign as f64 * 2.0 * PI / (1u64 << k) as f64;
        let phase = Complex::from_polar(1.0, angle);

        let control_bit = 1usize << control;
        let target_bit = 1usize << target;

        for (i, s) in state.iter_mut().enumerate().take(size) {
            if (i & control_bit) != 0 && (i & target_bit) != 0 {
                *s = *s * phase;
            }
        }
    }

    /// Apply SWAP gate between two qubits.
    fn apply_swap(state: &mut [Complex], qubit_a: usize, qubit_b: usize, n: usize) {
        let size = 1usize << n;
        let bit_a = 1usize << qubit_a;
        let bit_b = 1usize << qubit_b;

        for i in 0..size {
            let a_set = (i & bit_a) != 0;
            let b_set = (i & bit_b) != 0;
            // Only swap when the two qubit values differ
            if a_set != b_set {
                let j = i ^ bit_a ^ bit_b;
                if i < j {
                    state.swap(i, j);
                }
            }
        }
    }
}
