// SP1 Host Program — orchestrates proof generation, classical verification,
// and artifact output for the Oathbreaker ZKP.
//
// Three modes of operation:
//   1. Classical (default): Build circuit + verify test cases without SP1
//   2. Execute (--features sp1, --mode execute): Run guest in SP1 without proof
//   3. Prove (--features sp1, --mode prove): Generate Groth16 SNARK proof

use clap::Parser;
use ec_goldilocks::curve::CurveParams;
use ec_goldilocks::double_scalar_mul;
use ec_goldilocks::params::load_all_curve_params;
use ec_goldilocks::point_ops::scalar_mul;
use ec_goldilocks::test_case::{ProofInput, ProofOutput, TestCase};
use group_action_circuit::build_group_action_circuit_jacobian;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// When compiled with --features sp1, include the guest program ELF.
#[cfg(feature = "sp1")]
sp1_sdk::include_elf!("sp1-program");

/// Oathbreaker SP1 Host — ZK proof generation for quantum circuit verification.
#[derive(Parser, Debug)]
#[command(
    name = "sp1-host",
    about = "Generate and verify ZK proofs for the Oathbreaker circuit"
)]
struct Args {
    /// Oath curve tier to use.
    #[arg(long, default_value = "oath-8")]
    tier: String,

    /// Number of random test cases to verify.
    #[arg(long, default_value_t = 10)]
    num_cases: usize,

    /// Execution mode: classical, execute, or prove.
    ///
    /// - classical: Build circuit and verify locally (no SP1 needed)
    /// - execute: Run guest in SP1 zkVM without proof generation (fast)
    /// - prove: Generate a Groth16 SNARK proof (slow, requires SP1 toolchain)
    #[arg(long, default_value = "classical")]
    mode: String,

    /// Directory for proof artifacts (proof.bin, vk.bin, circuit_summary.json).
    #[arg(long, default_value = "../../proofs")]
    output_dir: PathBuf,

    /// Path to the combined curve parameters JSON file.
    #[arg(long, default_value = "../../sage/oath_all_params.json")]
    params_file: PathBuf,

    /// Cross-verify a subset of test cases with Pollard's rho ECDLP solver.
    /// Only practical for small tiers (Oath-8, Oath-16).
    #[arg(long, default_value_t = false)]
    cross_verify: bool,
}

/// Select window size based on field bit size.
/// w=4 for small fields (<=16 bits), w=8 for larger fields.
fn window_size_for_field(field_bits: usize) -> usize {
    if field_bits <= 16 {
        4
    } else {
        8
    }
}

/// Resolve the params file path relative to the sp1-host crate directory.
fn resolve_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        // When run via `cargo run -p sp1-host`, the working directory is
        // the workspace root (oathbreaker/). Adjust relative paths.
        let candidates = [
            path.to_path_buf(),
            PathBuf::from("sage/oath_all_params.json"),
            PathBuf::from("crates/sp1-host").join(path),
        ];
        for candidate in &candidates {
            if candidate.exists() {
                return candidate.clone();
            }
        }
        path.to_path_buf()
    }
}

/// Load curve parameters for the requested tier.
fn load_tier_params(params_path: &Path, tier: &str) -> Result<CurveParams, String> {
    let resolved = resolve_path(params_path);
    let all_params = load_all_curve_params(&resolved)?;

    // Normalize tier name: "oath-8" -> "Oath-8", "oath8" -> "Oath-8"
    let normalized = tier
        .to_lowercase()
        .replace("oath-", "oath_")
        .replace("oath", "Oath-")
        .replace("Oath-_", "Oath-");

    for (name, params) in &all_params {
        let name_normalized = name
            .to_lowercase()
            .replace("oath-", "oath_")
            .replace("oath", "Oath-")
            .replace("Oath-_", "Oath-");
        if name_normalized == normalized
            || name.to_lowercase() == tier.to_lowercase()
            || name.to_lowercase().replace('-', "") == tier.to_lowercase().replace('-', "")
        {
            return Ok(params.clone());
        }
    }

    // Fall back to matching by field_bits
    let target_bits: Option<usize> = tier
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .ok();

    if let Some(bits) = target_bits {
        for (_, params) in &all_params {
            if params.field_bits == bits {
                return Ok(params.clone());
            }
        }
    }

    Err(format!(
        "Tier '{}' not found. Available tiers: {}",
        tier,
        all_params
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// Generate random test cases for the group-action circuit.
fn generate_test_cases(curve: &CurveParams, count: usize) -> Vec<TestCase> {
    let mut rng = rand::thread_rng();
    let order = curve.order.max(2); // Guard against degenerate curves

    (0..count)
        .map(|_| {
            let a = rng.gen_range(1..order);
            let b = rng.gen_range(1..order);
            let k = rng.gen_range(1..order);
            let target_q = scalar_mul(k, &curve.generator, curve);
            let expected = double_scalar_mul(a, &curve.generator, b, &target_q, curve);
            TestCase {
                a,
                b,
                target_q,
                expected,
            }
        })
        .collect()
}

/// Cross-verify test cases using Pollard's rho ECDLP solver.
fn cross_verify_with_pollard(curve: &CurveParams, test_cases: &[TestCase]) {
    println!("  Cross-verifying with Pollard's rho...");
    let mut verified = 0;
    for (i, case) in test_cases.iter().take(5).enumerate() {
        match ec_goldilocks::ecdlp::pollard_rho(&curve.generator, &case.target_q, curve) {
            Some(k_recovered) => {
                let q_check = scalar_mul(k_recovered, &curve.generator, curve);
                if q_check == case.target_q {
                    verified += 1;
                } else {
                    eprintln!(
                        "  Warning: Pollard's rho case {} — recovered k doesn't match",
                        i
                    );
                }
            }
            None => {
                eprintln!(
                    "  Warning: Pollard's rho failed on case {} (may need retry)",
                    i
                );
            }
        }
    }
    println!(
        "  Pollard's rho: {}/{} verified",
        verified,
        test_cases.len().min(5)
    );
}

/// Print a ProofOutput summary.
fn print_output(output: &ProofOutput) {
    println!("\n  Circuit Resource Counts (Proven):");
    println!("  ├── Field bits:      {}", output.field_bits);
    println!("  ├── Window size:     {}", output.window_size);
    println!("  ├── Logical qubits:  {}", output.qubit_count);
    println!("  ├── Toffoli gates:   {}", output.toffoli_count);
    println!("  ├── CNOT gates:      {}", output.cnot_count);
    println!("  ├── Circuit depth:   {}", output.depth);
    println!("  ├── Tests verified:  {}", output.num_test_cases);
    println!(
        "  └── Circuit hash:    {}",
        output
            .circuit_hash
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>()
    );
}

/// Classical mode: build circuit and verify test cases without SP1.
fn run_classical(input: &ProofInput) -> ProofOutput {
    println!(
        "[3/4] Building group-action circuit (Jacobian, w={})...",
        input.window_size
    );
    let circuit = build_group_action_circuit_jacobian(&input.curve, input.window_size);
    let summary = circuit.summary();

    // Compute circuit hash
    let summary_json = serde_json::to_vec(&summary).expect("Failed to serialize CircuitSummary");
    let mut hasher = Sha256::new();
    hasher.update(&summary_json);
    let hash_result = hasher.finalize();
    let mut circuit_hash = [0u8; 32];
    circuit_hash.copy_from_slice(&hash_result);

    println!(
        "[4/4] Verifying {} test cases against classical reference...",
        input.test_cases.len()
    );
    for (i, case) in input.test_cases.iter().enumerate() {
        let result = circuit.execute_classical(case.a, case.b, &case.target_q);
        assert!(
            result == case.expected,
            "Test case {} failed: circuit output != expected",
            i
        );
    }
    println!("  All {} test cases passed.", input.test_cases.len());

    ProofOutput {
        qubit_count: summary.logical_qubits_peak,
        toffoli_count: summary.toffoli_gates,
        cnot_count: summary.cnot_gates,
        depth: summary.circuit_depth,
        num_test_cases: input.test_cases.len(),
        field_bits: input.curve.field_bits,
        window_size: input.window_size,
        circuit_hash,
    }
}

/// SP1 execute mode: run the guest program in the zkVM without proof generation.
/// This validates the guest logic quickly (seconds for small tiers).
#[cfg(feature = "sp1")]
fn run_sp1_execute(input: &ProofInput) -> ProofOutput {
    use sp1_sdk::ProverClient;

    println!("[3/5] Initializing SP1 prover client...");
    let client = ProverClient::from_env();

    println!("[4/5] Executing guest program in SP1 zkVM (no proof)...");
    let mut stdin = sp1_sdk::SP1Stdin::new();
    stdin.write(input);

    let (output, report) = client
        .execute(SP1_PROGRAM_ELF, &stdin)
        .run()
        .expect("SP1 execution failed");

    println!(
        "  Execution complete: {} cycles",
        report.total_instruction_count()
    );

    let proof_output: ProofOutput = output.read();

    println!("[5/5] Guest program verified all test cases.");
    proof_output
}

/// SP1 prove mode: generate a Groth16 SNARK proof.
#[cfg(feature = "sp1")]
fn run_sp1_prove(input: &ProofInput, output_dir: &Path) -> ProofOutput {
    use sp1_sdk::ProverClient;

    println!("[3/7] Initializing SP1 prover client...");
    let client = ProverClient::from_env();

    println!("[4/7] Setting up proving and verification keys...");
    let (pk, vk) = client.setup(SP1_PROGRAM_ELF);

    let mut stdin = sp1_sdk::SP1Stdin::new();
    stdin.write(input);

    println!("[5/7] Generating Groth16 SNARK proof (this may take a while)...");
    let proof = client
        .prove(&pk, &stdin)
        .groth16()
        .run()
        .expect("Proof generation failed");

    println!("[6/7] Verifying proof...");
    client
        .verify(&proof, &vk)
        .expect("Proof verification failed — this should never happen");
    println!("  Proof verified successfully.");

    // Read public values from the proof
    let proof_output: ProofOutput = proof.public_values.read();

    // Write artifacts
    println!(
        "[7/7] Writing proof artifacts to {}...",
        output_dir.display()
    );
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");

    std::fs::write(
        output_dir.join("proof.bin"),
        bincode::serialize(&proof).expect("Failed to serialize proof"),
    )
    .expect("Failed to write proof.bin");

    std::fs::write(
        output_dir.join("vk.bin"),
        bincode::serialize(&vk).expect("Failed to serialize verification key"),
    )
    .expect("Failed to write vk.bin");

    std::fs::write(
        output_dir.join("circuit_summary.json"),
        serde_json::to_string_pretty(&proof_output).expect("Failed to serialize output"),
    )
    .expect("Failed to write circuit_summary.json");

    println!("  Artifacts written:");
    println!("    proof.bin           — Groth16 SNARK proof");
    println!("    vk.bin              — Verification key");
    println!("    circuit_summary.json — Public values (resource counts + circuit hash)");

    proof_output
}

fn main() {
    let args = Args::parse();

    println!("=== Oathbreaker SP1 Host ===\n");
    println!("Tier:       {}", args.tier);
    println!("Test cases: {}", args.num_cases);
    println!("Mode:       {}", args.mode);
    println!();

    // Step 1: Load curve parameters
    println!("[1/{}] Loading curve parameters...", step_count(&args.mode));
    let curve = match load_tier_params(&args.params_file, &args.tier) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    println!(
        "  Loaded {}-bit curve: a={}, b={}, order={}",
        curve.field_bits, curve.a, curve.b, curve.order
    );

    // Step 2: Generate test cases
    let window_size = window_size_for_field(curve.field_bits);
    println!(
        "[2/{}] Generating {} random test cases (w={})...",
        step_count(&args.mode),
        args.num_cases,
        window_size
    );
    let test_cases = generate_test_cases(&curve, args.num_cases);

    // Optional: cross-verify with Pollard's rho
    if args.cross_verify && curve.field_bits <= 16 {
        cross_verify_with_pollard(&curve, &test_cases);
    }

    let input = ProofInput {
        curve: curve.clone(),
        window_size,
        test_cases,
    };

    // Dispatch to the appropriate mode
    let output = match args.mode.as_str() {
        "classical" => {
            let result = run_classical(&input);
            println!("\n  To generate a ZK proof, install the SP1 toolchain and run:");
            println!(
                "    cargo run --release -p sp1-host --features sp1 -- --tier {} --mode prove",
                args.tier
            );
            result
        }
        #[cfg(feature = "sp1")]
        "execute" => run_sp1_execute(&input),
        #[cfg(feature = "sp1")]
        "prove" => run_sp1_prove(&input, &args.output_dir),
        #[cfg(not(feature = "sp1"))]
        "execute" | "prove" => {
            eprintln!("Error: SP1 modes require the `sp1` feature.");
            eprintln!(
                "Rebuild with: cargo run -p sp1-host --features sp1 -- --mode {}",
                args.mode
            );
            std::process::exit(1);
        }
        other => {
            eprintln!(
                "Error: Unknown mode '{}'. Use: classical, execute, or prove",
                other
            );
            std::process::exit(1);
        }
    };

    print_output(&output);
    println!("\nDone.");
}

/// Return the total step count for progress display.
fn step_count(mode: &str) -> usize {
    match mode {
        "classical" => 4,
        "execute" => 5,
        "prove" => 7,
        _ => 4,
    }
}
