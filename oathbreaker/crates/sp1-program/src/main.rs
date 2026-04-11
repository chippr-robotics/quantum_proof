// SP1 Guest Program — runs INSIDE the zkVM; every operation is proven.
//
// Uncomment the sp1_zkvm lines when the SP1 toolchain is installed.
// For now, this serves as the specification of what the guest proves.

// #![no_main]
// sp1_zkvm::entrypoint!(main);

use ec_goldilocks::curve::{AffinePoint, CurveParams};
use group_action_circuit::build_group_action_circuit;

/// Window size for the group-action circuit.
const WINDOW_SIZE: usize = 8;

fn main() {
    // In the SP1 guest, inputs are read from the host via sp1_zkvm::io::read.
    // For now, this is a placeholder showing the intended flow.

    // --- Read inputs from host ---
    // let curve_params: CurveParams = sp1_zkvm::io::read();
    // let test_cases: Vec<TestCase> = sp1_zkvm::io::read();

    // --- Build the group-action circuit ---
    // let circuit = build_group_action_circuit(&curve_params, WINDOW_SIZE);

    // --- Commit circuit resource counts (this is what we're proving) ---
    // sp1_zkvm::io::commit(&circuit.qubit_count());
    // sp1_zkvm::io::commit(&circuit.toffoli_count());
    // sp1_zkvm::io::commit(&circuit.cnot_count());
    // sp1_zkvm::io::commit(&circuit.depth());

    // --- Execute circuit on each test case, verify against classical reference ---
    // for case in &test_cases {
    //     // Classical reference: [a]G + [b]Q
    //     let expected = ec_goldilocks::double_scalar_mul(
    //         case.a, &curve_params.generator,
    //         case.b, &case.target_q,
    //         &curve_params,
    //     );
    //     // Circuit execution on basis state
    //     let result = circuit.execute_classical(case.a, case.b, &case.target_q);
    //     assert_eq!(result, expected, "Circuit output mismatch");
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
