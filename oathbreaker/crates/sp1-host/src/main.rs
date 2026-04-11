// SP1 Host Program — generates proofs and writes artifacts.
//
// Uncomment the sp1_sdk lines when the SP1 toolchain is installed.

use ec_goldilocks::curve::{AffinePoint, CurveParams};
use ec_goldilocks::ecdlp;
use ec_goldilocks::point_ops::scalar_mul;
use rand::Rng;

/// Number of random test cases to include in the proof.
const NUM_TEST_CASES: usize = 100;

/// Window size (must match sp1-program).
const WINDOW_SIZE: usize = 8;

fn main() {
    println!("=== Oathbreaker SP1 Host ===\n");

    // --- Load Oath-64 curve parameters ---
    // TODO: Load from sage/oath64_params.json once generated
    // let curve = load_curve_params("oath64_params.json");
    println!("[1/5] Loading Oath-64 curve parameters... (awaiting Sage generation)");

    // --- Generate test cases for double-scalar verification ---
    println!("[2/5] Generating {} random test cases...", NUM_TEST_CASES);
    // Each test case: (a, b, k, Q=[k]G, expected=[a]G+[b]Q)
    // let mut rng = rand::thread_rng();
    // let test_cases: Vec<TestCase> = (0..NUM_TEST_CASES)
    //     .map(|_| {
    //         let a: u64 = rng.gen_range(1..curve.order);
    //         let b: u64 = rng.gen_range(1..curve.order);
    //         let k: u64 = rng.gen_range(1..curve.order);
    //         let q = scalar_mul(k, &curve.generator, &curve);
    //         let expected = ec_goldilocks::double_scalar_mul(
    //             a, &curve.generator, b, &q, &curve,
    //         );
    //         TestCase { a, b, target_q: q, expected }
    //     })
    //     .collect();

    // --- Verify via Pollard's rho on a subset ---
    println!("[3/5] Cross-verifying ECDLP instances with Pollard's rho...");
    // for case in test_cases.iter().take(10) {
    //     let k_recovered = ecdlp::pollard_rho(&curve.generator, &case.target_q, &curve)
    //         .expect("Pollard's rho failed");
    //     let q_check = scalar_mul(k_recovered, &curve.generator, &curve);
    //     assert_eq!(q_check, case.target_q, "Pollard's rho mismatch");
    // }

    // --- Generate SP1 proof ---
    println!("[4/5] Generating SP1 Groth16 proof...");
    // let client = sp1_sdk::ProverClient::new();
    // let (pk, vk) = client.setup(SP1_PROGRAM_ELF);
    //
    // let mut stdin = sp1_sdk::SP1Stdin::new();
    // stdin.write(&curve);
    // stdin.write(&test_cases);
    //
    // let proof = client.prove(&pk, stdin)
    //     .groth16()
    //     .run()
    //     .expect("Proof generation failed");
    //
    // client.verify(&proof, &vk).expect("Proof verification failed");

    // --- Write artifacts ---
    println!("[5/5] Writing proof artifacts...");
    // std::fs::write("proofs/oath64_proof.bin", proof.bytes())
    //     .expect("Failed to write proof");
    // std::fs::write("proofs/oath64_vk.bin", vk.bytes())
    //     .expect("Failed to write verification key");

    println!("\nSP1 host stub — awaiting SP1 toolchain installation");
    println!("Once installed, run: cargo run --release -p sp1-host");
}
