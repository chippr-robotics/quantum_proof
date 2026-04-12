use crate::curve::{AffinePoint, CurveParams};
use serde::{Deserialize, Serialize};

/// A single test case for the group-action circuit verification.
///
/// The SP1 guest proves that the circuit correctly computes
/// \[a\]G + \[b\]Q for each test case, matching the classical reference.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestCase {
    /// Scalar for the generator G.
    pub a: u64,
    /// Scalar for the target point Q.
    pub b: u64,
    /// Target point Q = \[k\]G (the "public key" being attacked).
    pub target_q: AffinePoint,
    /// Expected result: \[a\]G + \[b\]Q (classical reference).
    pub expected: AffinePoint,
}

/// Input data sent from the host to the SP1 guest via SP1Stdin.
///
/// Contains everything the guest needs to build the circuit and
/// verify it against classical ground truth.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofInput {
    /// Curve parameters (Oath-N tier).
    pub curve: CurveParams,
    /// Window size for circuit construction.
    pub window_size: usize,
    /// Test cases to verify inside the zkVM.
    pub test_cases: Vec<TestCase>,
}

/// Public values committed by the guest program.
///
/// These values are readable by anyone who verifies the proof,
/// without access to the circuit internals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProofOutput {
    /// Peak logical qubit count of the constructed circuit.
    pub qubit_count: usize,
    /// Total Toffoli gates in the circuit.
    pub toffoli_count: usize,
    /// Total CNOT gates in the circuit.
    pub cnot_count: usize,
    /// Circuit depth (critical path length).
    pub depth: usize,
    /// Number of test cases successfully verified.
    pub num_test_cases: usize,
    /// Field size in bits (identifies the Oath-N tier).
    pub field_bits: usize,
    /// Window size used for circuit construction.
    pub window_size: usize,
    /// SHA-256 hash of the serialized CircuitSummary JSON.
    pub circuit_hash: [u8; 32],
}
