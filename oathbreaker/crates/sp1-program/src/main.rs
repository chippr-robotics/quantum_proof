// SP1 Guest Program — runs INSIDE the zkVM; every operation is proven.
//
// Uncomment the sp1_zkvm lines when the SP1 toolchain is installed.
// For now, this serves as the specification of what the guest proves.

// #![no_main]
// sp1_zkvm::entrypoint!(main);

use ec_goldilocks::curve::{AffinePoint, CurveParams};
use shor_circuit::build_shor_circuit;

/// Window size for the Shor circuit.
const WINDOW_SIZE: usize = 8;

fn main() {
    // In the SP1 guest, inputs are read from the host via sp1_zkvm::io::read.
    // For now, this is a placeholder showing the intended flow.

    // --- Read inputs from host ---
    // let curve_params: CurveParams = sp1_zkvm::io::read();
    // let test_cases: Vec<(u64, AffinePoint)> = sp1_zkvm::io::read();

    // --- Build the Shor circuit ---
    // let circuit = build_shor_circuit(&curve_params, WINDOW_SIZE);

    // --- Commit circuit resource counts (this is what we're proving) ---
    // sp1_zkvm::io::commit(&circuit.qubit_count());
    // sp1_zkvm::io::commit(&circuit.toffoli_count());
    // sp1_zkvm::io::commit(&circuit.cnot_count());
    // sp1_zkvm::io::commit(&circuit.depth());

    // --- Run circuit on each test case, verify output ---
    // for (k, expected_q) in &test_cases {
    //     let result = circuit.execute_classical(*k);
    //     assert_eq!(result, *expected_q, "Circuit output mismatch for k={}", k);
    // }

    // --- Commit hash of the circuit (for identification) ---
    // use sha2::{Sha256, Digest};
    // let circuit_hash = {
    //     let serialized = serde_json::to_vec(&circuit.summary()).unwrap();
    //     let mut hasher = Sha256::new();
    //     hasher.update(&serialized);
    //     hasher.finalize().to_vec()
    // };
    // sp1_zkvm::io::commit_slice(&circuit_hash);

    println!("SP1 guest program stub — awaiting SP1 toolchain installation");
}
