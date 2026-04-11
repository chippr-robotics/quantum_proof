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

    // --- Load curve parameters ---
    // TODO: Load Oath-64 curve from sage/curve_params.json once generated
    // let curve = load_curve_params("curve_params.json");
    println!("[1/5] Loading Oath-64 curve parameters... (awaiting Sage generation)");

    // --- Generate test cases ---
    println!("[2/5] Generating {} random test cases...", NUM_TEST_CASES);
    // let mut rng = rand::thread_rng();
    // let test_cases: Vec<(u64, AffinePoint)> = (0..NUM_TEST_CASES)
    //     .map(|_| {
    //         let k: u64 = rng.gen_range(1..curve.order);
    //         let q = scalar_mul(k, &curve.generator, &curve);
    //         (k, q)
    //     })
    //     .collect();

    // --- Verify via Pollard's rho ---
    println!("[3/5] Cross-verifying with Pollard's rho...");
    // for (k, q) in test_cases.iter().take(10) {
    //     let k_recovered = ecdlp::pollard_rho(&curve.generator, q, &curve)
    //         .expect("Pollard's rho failed");
    //     assert_eq!(*k, k_recovered, "Pollard's rho mismatch");
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
    // std::fs::write("proofs/oathbreaker_proof.bin", proof.bytes())
    //     .expect("Failed to write proof");
    // std::fs::write("proofs/verification_key.bin", vk.bytes())
    //     .expect("Failed to write verification key");

    println!("\nSP1 host stub — awaiting SP1 toolchain installation");
    println!("Once installed, run: cargo run --release -p sp1-host");
}
