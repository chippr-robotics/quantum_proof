// SP1 Guest Program — runs INSIDE the zkVM; every operation is proven.
//
// When compiled with the `sp1` feature (via the SP1 toolchain targeting RISC-V),
// this program is the guest: it reads inputs from the host, builds the circuit,
// verifies test cases, and commits public values as a ProofOutput.
//
// When compiled without the `sp1` feature (regular `cargo build`), this program
// runs the same verification logic as a standalone binary for testing.

#![cfg_attr(feature = "sp1", no_main)]

use ec_oath::curve::CurveParams;
use ec_oath::test_case::{ProofInput, ProofOutput};
use group_action_circuit::build_group_action_circuit_jacobian;
use sha2::{Digest, Sha256};

#[cfg(feature = "sp1")]
sp1_zkvm::entrypoint!(main);

/// Core verification logic shared between SP1 guest and classical modes.
///
/// Builds the group-action circuit, verifies all test cases against the
/// classical reference, and returns the ProofOutput with resource counts
/// and circuit hash.
fn verify_circuit(input: &ProofInput) -> ProofOutput {
    let curve = &input.curve;
    let window_size = input.window_size;

    // Build the coherent double-scalar group-action circuit.
    // This is the computationally dominant step — materializes all reversible
    // gates and counts resources (qubits, Toffoli, CNOT).
    let circuit = build_group_action_circuit_jacobian(curve, window_size);
    let summary = circuit.summary();

    // Compute SHA-256 hash of the circuit summary for identification.
    // This binds the proof to a specific circuit structure.
    let summary_json = serde_json::to_vec(&summary).expect("Failed to serialize CircuitSummary");
    let mut hasher = Sha256::new();
    hasher.update(&summary_json);
    let hash_result = hasher.finalize();
    let mut circuit_hash = [0u8; 32];
    circuit_hash.copy_from_slice(&hash_result);

    // Verify each test case: circuit's classical execution must match
    // the expected [a]G + [b]Q from the host.
    for (i, case) in input.test_cases.iter().enumerate() {
        let result = circuit.execute_classical(case.a, case.b, &case.target_q);
        assert!(
            result == case.expected,
            "Test case {} failed: circuit output != expected [a]G + [b]Q",
            i
        );
    }

    ProofOutput {
        qubit_count: summary.logical_qubits_peak,
        toffoli_count: summary.toffoli_gates,
        cnot_count: summary.cnot_gates,
        depth: summary.circuit_depth,
        num_test_cases: input.test_cases.len(),
        field_bits: curve.field_bits,
        window_size,
        circuit_hash,
    }
}

/// Read a ProofInput from the SP1 host and commit the ProofOutput.
#[cfg(feature = "sp1")]
pub fn main() {
    let input: ProofInput = sp1_zkvm::io::read();
    let output = verify_circuit(&input);
    sp1_zkvm::io::commit(&output);
}

/// Classical mode: build and verify a small test case without SP1.
#[cfg(not(feature = "sp1"))]
pub fn main() {
    println!("=== SP1 Guest Program (Classical Mode) ===\n");
    println!("This binary runs the same verification logic as the SP1 guest,");
    println!("but without generating a zero-knowledge proof.\n");
    println!("To generate a ZK proof, use the SP1 host program:");
    println!("  cargo run --release -p sp1-host --features sp1 -- --tier oath-8\n");

    // Run a minimal self-test with a hardcoded 8-bit curve.
    let curve = small_test_curve();
    let test_cases = generate_small_test_cases(&curve, 3);

    let input = ProofInput {
        curve,
        window_size: 4,
        test_cases,
    };

    println!(
        "Running self-test: {}-bit field, {} test cases, w={}...",
        input.curve.field_bits,
        input.test_cases.len(),
        input.window_size
    );

    let output = verify_circuit(&input);

    println!("  Qubits:      {}", output.qubit_count);
    println!("  Toffoli:     {}", output.toffoli_count);
    println!("  CNOT:        {}", output.cnot_count);
    println!("  Depth:       {}", output.depth);
    println!("  Tests passed: {}", output.num_test_cases);
    println!(
        "  Circuit hash: {}",
        output
            .circuit_hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    );
    println!("\nClassical self-test passed.");
}

/// Construct a minimal 8-bit test curve for self-testing.
/// Uses the Oath-8 parameters: y^2 = x^3 + x + 3 over GF(241).
#[cfg(not(feature = "sp1"))]
fn small_test_curve() -> CurveParams {
    use oath_field::GoldilocksField;

    CurveParams {
        a: GoldilocksField::new(1),
        b: GoldilocksField::new(3),
        order: 247, // #E(GF(241))
        generator: ec_oath::AffinePoint::new(
            GoldilocksField::new(2),
            GoldilocksField::new(75),
        ),
        field_bits: 8,
    }
}

/// Generate random test cases for the self-test.
#[cfg(not(feature = "sp1"))]
fn generate_small_test_cases(curve: &CurveParams, count: usize) -> Vec<ec_oath::TestCase> {
    use ec_oath::double_scalar_mul;
    use ec_oath::point_ops::scalar_mul;

    // Use deterministic "random" values for reproducibility.
    let scalars: Vec<(u64, u64, u64)> = (0..count)
        .map(|i| {
            let a = (i as u64 * 7 + 3) % curve.order.max(1);
            let b = (i as u64 * 13 + 5) % curve.order.max(1);
            let k = (i as u64 * 11 + 7) % curve.order.max(1);
            (a, b, k)
        })
        .collect();

    scalars
        .into_iter()
        .map(|(a, b, k)| {
            let target_q = scalar_mul(k, &curve.generator, curve);
            let expected = double_scalar_mul(a, &curve.generator, b, &target_q, curve);
            ec_oath::TestCase {
                a,
                b,
                target_q,
                expected,
            }
        })
        .collect()
}
