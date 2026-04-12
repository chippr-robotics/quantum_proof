mod comparison;
mod counter;
mod oath_tiers;
mod params;
mod scaling;

use group_action_circuit::export::{export_qasm, export_stats_json};
use std::path::PathBuf;

/// Resolve the project root (the `oathbreaker/` directory) from the current
/// working directory or the benchmark binary's location.
fn find_project_dir() -> PathBuf {
    // Try common locations relative to the working directory.
    let candidates = [
        PathBuf::from("sage"),             // cwd is oathbreaker/
        PathBuf::from("oathbreaker/sage"), // cwd is repo root
    ];
    for c in &candidates {
        if c.exists() {
            return c.parent().unwrap().to_path_buf();
        }
    }
    // Fallback: assume cwd is oathbreaker/
    PathBuf::from(".")
}

fn run_benchmarks() {
    println!("=== Oathbreaker Benchmark Suite ===\n");

    let project_dir = find_project_dir();
    let params_path = project_dir.join("sage/oath64_params.json");

    let curve = match params::load_curve_params(&params_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: {}", e);
            eprintln!("Falling back to hardcoded Oath-64 parameters.\n");
            hardcoded_oath64()
        }
    };

    let window_size = 8;

    // --- Affine circuit ---
    println!("=== Affine Coordinate Circuit (baseline) ===\n");
    let circuit_affine = group_action_circuit::build_group_action_circuit(&curve, window_size);
    counter::print_resource_table(&circuit_affine);
    println!();

    // --- Jacobian circuit (optimized) ---
    println!("=== Jacobian Coordinate Circuit (optimized) ===\n");
    let circuit_jacobian =
        group_action_circuit::build_group_action_circuit_jacobian(&curve, window_size);
    counter::print_resource_table(&circuit_jacobian);
    println!();

    // --- Improvement summary ---
    let affine_summary = circuit_affine.summary();
    let jac_summary = circuit_jacobian.summary();
    println!("=== Jacobian vs Affine Improvement ===\n");
    if affine_summary.toffoli_gates > 0 {
        let toffoli_ratio =
            affine_summary.toffoli_gates as f64 / jac_summary.toffoli_gates.max(1) as f64;
        println!(
            "  Toffoli reduction: {:.1}x ({} → {})",
            toffoli_ratio, affine_summary.toffoli_gates, jac_summary.toffoli_gates,
        );
    }
    println!(
        "  Field inversions: {} → {} (single final Fermat inversion)",
        affine_summary.field_inversions, jac_summary.field_inversions,
    );
    println!(
        "  Qubit overhead:   {} → {} (+{} for Z register)",
        affine_summary.logical_qubits_peak,
        jac_summary.logical_qubits_peak,
        jac_summary
            .logical_qubits_peak
            .saturating_sub(affine_summary.logical_qubits_peak),
    );
    println!();

    // --- Scaling projections (based on the optimized Jacobian circuit) ---
    println!("=== Scaling Projections (from Jacobian Oath-64 baseline) ===\n");
    let projections = scaling::project_scaling(
        circuit_jacobian.qubit_count(),
        circuit_jacobian.toffoli_count(),
        64,
    );
    scaling::print_scaling_table(&projections);
    println!();

    // --- Comparison to prior work ---
    let projection_256 = projections
        .iter()
        .find(|p| p.field_bits == 256)
        .map(|p| (p.projected_qubits, p.projected_toffoli));
    comparison::print_comparison_table(projection_256);
    println!();

    // --- Oath-N benchmark tiers ---
    oath_tiers::print_oath_tiers();

    // --- JSON summary ---
    println!("\n=== JSON Circuit Summary (Jacobian, Oath-64) ===\n");
    println!("{}", export_stats_json(&circuit_jacobian));
}

fn run_export_qasm(output_path: Option<String>) {
    let project_dir = find_project_dir();
    let params_path = project_dir.join("sage/oath64_params.json");

    let curve = match params::load_curve_params(&params_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: {}", e);
            eprintln!("Falling back to hardcoded Oath-64 parameters.\n");
            hardcoded_oath64()
        }
    };

    let window_size = 8;

    eprintln!(
        "Building Jacobian group-action circuit (Oath-64, window={})...",
        window_size
    );
    let circuit = group_action_circuit::build_group_action_circuit_jacobian(&curve, window_size);

    let summary = circuit.summary();
    eprintln!(
        "  Qubits: {}, Toffoli: {}, Total gates: {}",
        summary.logical_qubits_peak, summary.toffoli_gates, summary.total_reversible_gates,
    );

    let qasm = export_qasm(&circuit);

    let out = output_path.unwrap_or_else(|| "oathbreaker_oath64.qasm".to_string());
    match std::fs::write(&out, &qasm) {
        Ok(()) => {
            eprintln!("QASM written to: {}", out);
            eprintln!("  Lines: {}", qasm.lines().count());
            eprintln!("  Size:  {} bytes", qasm.len());
        }
        Err(e) => {
            eprintln!("Failed to write {}: {}", out, e);
            // Fall back to stdout
            print!("{}", qasm);
        }
    }

    // Also write the JSON stats alongside
    let stats_path = format!("{}.stats.json", out.trim_end_matches(".qasm"));
    let stats_json = export_stats_json(&circuit);
    if let Err(e) = std::fs::write(&stats_path, &stats_json) {
        eprintln!("Warning: could not write stats to {}: {}", stats_path, e);
    } else {
        eprintln!("Stats written to: {}", stats_path);
    }
}

fn run_export_all_qasm() {
    let project_dir = find_project_dir();
    let all_params_path = project_dir.join("sage/oath_all_params.json");

    let all_curves = match params::load_all_curve_params(&all_params_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Cannot export all tiers without oath_all_params.json");
            std::process::exit(1);
        }
    };

    let window_size = 4; // smaller window for smaller curves

    for (tier_name, curve) in &all_curves {
        let w = if curve.field_bits >= 32 {
            8
        } else {
            window_size
        };
        eprintln!(
            "Building {} circuit ({}-bit, window={})...",
            tier_name, curve.field_bits, w
        );
        let circuit = group_action_circuit::build_group_action_circuit_jacobian(curve, w);
        let qasm = export_qasm(&circuit);
        let filename = format!(
            "oathbreaker_{}.qasm",
            tier_name.to_lowercase().replace('-', "")
        );
        match std::fs::write(&filename, &qasm) {
            Ok(()) => eprintln!("  Written: {} ({} lines)", filename, qasm.lines().count()),
            Err(e) => eprintln!("  Error writing {}: {}", filename, e),
        }
    }
}

/// Hardcoded Oath-64 parameters matching sage/oath64_params.json.
/// Used as fallback when the JSON file is not found.
///
/// Note: The true curve order (18446744077729562113) exceeds u64::MAX.
/// Circuit construction only uses field_bits, not the order, so this is fine.
/// The truncated order is stored for CurveParams compatibility.
fn hardcoded_oath64() -> ec_goldilocks::CurveParams {
    use goldilocks_field::GoldilocksField;

    // True order: 18_446_744_077_729_562_113 (exceeds u64::MAX)
    // Truncated: 18_446_744_077_729_562_113 mod 2^64
    const ORDER_FULL: u128 = 18_446_744_077_729_562_113;
    let order = (ORDER_FULL % (u64::MAX as u128 + 1)) as u64;

    ec_goldilocks::CurveParams {
        a: GoldilocksField::new(1),
        b: GoldilocksField::new(38),
        order,
        generator: ec_goldilocks::AffinePoint::new(
            GoldilocksField::new(1),
            GoldilocksField::new(4_519_977_769_586_765_578),
        ),
        field_bits: 64,
    }
}

fn print_usage() {
    eprintln!("Usage: benchmark [COMMAND]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  (default)          Run the full benchmark suite");
    eprintln!("  export-qasm [FILE] Export Oath-64 circuit as OpenQASM 3.0");
    eprintln!("  export-all-qasm    Export all Oath-N tiers as QASM files");
    eprintln!("  help               Show this help message");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(|s| s.as_str()) {
        None => run_benchmarks(),
        Some("export-qasm") => run_export_qasm(args.get(2).cloned()),
        Some("export-all-qasm") => run_export_all_qasm(),
        Some("help") | Some("--help") | Some("-h") => print_usage(),
        Some(other) => {
            eprintln!("Unknown command: {}", other);
            print_usage();
            std::process::exit(1);
        }
    }
}
