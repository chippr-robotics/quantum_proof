use reversible_arithmetic::register::QuantumRegister;
use reversible_arithmetic::resource_counter::ResourceCounter;

/// Phase estimation register management for Shor's ECDLP algorithm.
///
/// In Shor's algorithm for ECDLP, phase estimation extracts the discrete
/// logarithm from the eigenvalue phase of the scalar multiplication operator.
///
/// Register allocation:
/// - Scalar register: n qubits (initialized in superposition for quantum execution)
/// - Point X register: n qubits (holds x-coordinate of EC point)
/// - Point Y register: n qubits (holds y-coordinate of EC point)
/// - Ancilla registers: dynamically allocated by subroutines
pub struct PhaseEstimation {
    /// Number of bits in the scalar / field elements.
    pub num_bits: usize,
    /// The scalar register (phase estimation input).
    pub scalar_register: QuantumRegister,
    /// The point X-coordinate register.
    pub point_x_register: QuantumRegister,
    /// The point Y-coordinate register.
    pub point_y_register: QuantumRegister,
}

impl PhaseEstimation {
    /// Allocate all primary registers.
    pub fn new(num_bits: usize, counter: &mut ResourceCounter) -> Self {
        let scalar = QuantumRegister::new("scalar", num_bits);
        let point_x = QuantumRegister::new("point_x", num_bits);
        let point_y = QuantumRegister::new("point_y", num_bits);

        // 3 * num_bits primary qubits
        counter.allocate_qubits(3 * num_bits);

        Self {
            num_bits,
            scalar_register: scalar,
            point_x_register: point_x,
            point_y_register: point_y,
        }
    }

    /// Total primary (non-ancilla) qubits.
    pub fn primary_qubits(&self) -> usize {
        3 * self.num_bits
    }

    /// Initialize the point register with the generator point coordinates.
    pub fn load_generator(&mut self, gx: u64, gy: u64) {
        self.point_x_register.load_u64(gx);
        self.point_y_register.load_u64(gy);
    }

    /// For classical simulation: load a specific scalar value.
    pub fn load_scalar(&mut self, k: u64) {
        self.scalar_register.load_u64(k);
    }
}
