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
    let candidates = [
        PathBuf::from("sage"),             // cwd is oathbreaker/
        PathBuf::from("oathbreaker/sage"), // cwd is repo root
    ];
    for c in &candidates {
        if c.exists() {
            return c.parent().unwrap().to_path_buf();
        }
    }
    PathBuf::from(".")
}

/// Choose an appropriate window size for a given field size.
/// Window must divide field_bits. Larger windows reduce iterations
/// but increase QROM table size exponentially.
fn window_for_field(field_bits: usize) -> usize {
    match field_bits {
        8 => 4,  // 2 windows, 16-entry table
        16 => 4, // 4 windows, 16-entry table
        32 => 8, // 4 windows, 256-entry table
        64 => 8, // 8 windows, 256-entry table
        _ => 4,  // conservative default
    }
}

fn run_benchmarks() {
    println!("=== Oathbreaker Benchmark Suite ===\n");

    let project_dir = find_project_dir();
    let all_params_path = project_dir.join("sage/oath_all_params.json");

    let all_curves = match params::load_all_curve_params(&all_params_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Warning: {}", e);
            eprintln!("Falling back to hardcoded Oath-64 parameters only.\n");
            vec![("Oath-64".to_string(), hardcoded_oath64())]
        }
    };

    // -----------------------------------------------------------------------
    // Phase 1: Build the Oath-8 circuit in BOTH coordinate systems to show
    //          the affine-vs-Jacobian improvement.
    // -----------------------------------------------------------------------
    if let Some((_, curve_8)) = all_curves.iter().find(|(n, _)| n == "Oath-8") {
        let w = window_for_field(curve_8.field_bits);
        println!(
            "=== Oath-8 Circuit ({}-bit field, window={}) ===\n",
            curve_8.field_bits, w
        );

        println!("--- Affine Coordinate Circuit (baseline) ---\n");
        let affine = group_action_circuit::build_group_action_circuit(curve_8, w);
        counter::print_resource_table(&affine);
        println!();

        println!("--- Jacobian Coordinate Circuit (optimized) ---\n");
        let jacobian = group_action_circuit::build_group_action_circuit_jacobian(curve_8, w);
        counter::print_resource_table(&jacobian);
        println!();

        let a = affine.summary();
        let j = jacobian.summary();
        println!("--- Jacobian vs Affine Improvement ---\n");
        if a.toffoli_gates > 0 {
            let ratio = a.toffoli_gates as f64 / j.toffoli_gates.max(1) as f64;
            println!(
                "  Toffoli reduction: {:.1}x ({} -> {})",
                ratio, a.toffoli_gates, j.toffoli_gates,
            );
        }
        println!(
            "  Field inversions: {} -> {} (single final Fermat inversion)",
            a.field_inversions, j.field_inversions,
        );
        println!(
            "  Qubit overhead:   {} -> {} (+{} for Z register)",
            a.logical_qubits_peak,
            j.logical_qubits_peak,
            j.logical_qubits_peak.saturating_sub(a.logical_qubits_peak),
        );
        println!();
    }

    // -----------------------------------------------------------------------
    // Phase 2: Build Jacobian v3 circuits for each measurable tier.
    //          v3 uses modified Jacobian doubling (-2 squarings/doubling),
    //          fixed KaratsubaSquarer delegation, and tighter register
    //          layout (12n+2 workspace vs 14n+2 in v2).
    //
    //          The circuit construction materializes all gate objects in
    //          memory (~O(n^3) gates at ~32 bytes each). Practical limits:
    //            Oath-8:   ~400K gates  (~13 MB)   -- instant
    //            Oath-16:  ~2M gates    (~64 MB)   -- fast
    //            Oath-32:  ~15M gates   (~480 MB)  -- moderate
    //            Oath-64:  ~90M+ gates  (~3+ GB)   -- exceeds CI runners
    // -----------------------------------------------------------------------
    println!("=== Measured Jacobian v3 Circuit Resources ===\n");

    let measurable_tiers: Vec<&str> = vec!["Oath-8", "Oath-16", "Oath-32"];
    let mut measured: Vec<(String, group_action_circuit::CircuitSummary)> = Vec::new();

    for tier_name in &measurable_tiers {
        if let Some((_, curve)) = all_curves.iter().find(|(n, _)| n == *tier_name) {
            let w = window_for_field(curve.field_bits);
            println!(
                "Building {} (Jacobian v3, {}-bit, window={})...",
                tier_name, curve.field_bits, w
            );
            let circuit = group_action_circuit::build_group_action_circuit_jacobian_v3(curve, w);
            let summary = circuit.summary();
            counter::print_resource_table(&circuit);
            // Print cost attribution for the largest tier
            if let Some(ref attr) = summary.cost_attribution {
                let total = summary.toffoli_gates.max(1);
                println!();
                println!("  Toffoli Cost Attribution:");
                println!("  ┌─────────────────────┬──────────────┬────────┐");
                println!("  │ Subsystem           │ Toffoli      │ Share  │");
                println!("  ├─────────────────────┼──────────────┼────────┤");
                let rows = [
                    ("Doublings", attr.doubling_toffoli),
                    ("QROM decode/load", attr.qrom_toffoli),
                    ("Mixed additions", attr.addition_toffoli),
                    ("Inversion (BGCD)", attr.inversion_toffoli),
                    ("Affine recovery", attr.affine_recovery_toffoli),
                ];
                let mut accounted = 0;
                for (name, cost) in &rows {
                    let pct = *cost as f64 / total as f64 * 100.0;
                    println!("  │ {:<19} │ {:<12} │ {:>5.1}% │", name, cost, pct,);
                    accounted += cost;
                }
                let other = total.saturating_sub(accounted);
                let other_pct = other as f64 / total as f64 * 100.0;
                println!(
                    "  │ {:<19} │ {:<12} │ {:>5.1}% │",
                    "Other/overhead", other, other_pct,
                );
                println!("  └─────────────────────┴──────────────┴────────┘");
            }
            println!();
            measured.push((tier_name.to_string(), summary));
        }
    }

    // -----------------------------------------------------------------------
    // Phase 2b: Window-size sweep.
    //           For each measured tier, try all valid window sizes and report
    //           the Toffoli / qubit tradeoff.  This determines whether the
    //           current default w is optimal under the new QROM + Karatsuba
    //           cost structure.
    // -----------------------------------------------------------------------
    println!("=== Window-Size Sweep ===\n");
    println!("┌───────────┬────────┬──────────────┬─────────────┬──────────────────┐");
    println!(
        "│ {:<9} │ {:<6} │ {:<12} │ {:<11} │ {:<16} │",
        "Tier", "Window", "Toffoli", "Qubits", "Total gates"
    );
    println!("├───────────┼────────┼──────────────┼─────────────┼──────────────────┤");

    // Track the best window per tier for use in projections
    let mut best_per_tier: Vec<(String, group_action_circuit::CircuitSummary)> = Vec::new();

    for tier_name in &measurable_tiers {
        if let Some((_, curve)) = all_curves.iter().find(|(n, _)| n == *tier_name) {
            let n = curve.field_bits;
            // Valid window sizes: must divide n, range 1..=n
            let valid_windows: Vec<usize> = (1..=n)
                .filter(|w| n % w == 0 && *w <= 8) // cap at 8 to keep QROM table ≤ 256
                .collect();

            let mut best_toffoli = usize::MAX;
            let mut best_summary = None;
            for &w in &valid_windows {
                let circuit = group_action_circuit::build_group_action_circuit_jacobian_v3(curve, w);
                let summary = circuit.summary();
                let marker = if summary.toffoli_gates < best_toffoli {
                    best_toffoli = summary.toffoli_gates;
                    best_summary = Some(summary.clone());
                    " ◄ best"
                } else {
                    ""
                };
                println!(
                    "│ {:<9} │ w={:<4} │ {:<12} │ {:<11} │ {:<16} │{}",
                    tier_name,
                    w,
                    summary.toffoli_gates,
                    summary.logical_qubits_peak,
                    summary.total_reversible_gates,
                    marker,
                );
            }

            if let Some(s) = best_summary {
                best_per_tier.push((tier_name.to_string(), s));
            }

            // Separator between tiers
            if tier_name != measurable_tiers.last().unwrap() {
                println!("├───────────┼────────┼──────────────┼─────────────┼──────────────────┤");
            }
        }
    }
    println!("└───────────┴────────┴──────────────┴─────────────┴──────────────────┘");
    println!();

    // Use the best-window results for the largest tier as our projection base
    if !best_per_tier.is_empty() {
        let (ref best_name, ref best_summary) = best_per_tier[best_per_tier.len() - 1];
        let default_w = window_for_field(best_summary.field_bits);
        let (_, ref default_summary) = measured[measured.len() - 1];

        if best_summary.toffoli_gates < default_summary.toffoli_gates {
            println!(
                "Window sweep found a better configuration for {}:",
                best_name
            );
            println!(
                "  Default (w={}): {} Toffoli, {} qubits",
                default_w, default_summary.toffoli_gates, default_summary.logical_qubits_peak,
            );
            println!(
                "  Best:           {} Toffoli, {} qubits",
                best_summary.toffoli_gates, best_summary.logical_qubits_peak,
            );
            println!(
                "  Improvement:    {:.1}% fewer Toffoli\n",
                (1.0 - best_summary.toffoli_gates as f64 / default_summary.toffoli_gates as f64)
                    * 100.0,
            );

            // Replace the measured entry with the best-window version
            if let Some(entry) = measured.last_mut() {
                *entry = (best_name.clone(), best_summary.clone());
            }
        } else {
            println!(
                "Default window (w={}) is already optimal for {}.\n",
                default_w, best_name,
            );
        }
    }

    // Note about Oath-64
    println!("Note: Oath-64 full circuit construction materializes ~90M+ gate objects");
    println!("      (~3 GB RAM) and is omitted from CI. Resource counts are projected");
    println!("      from measured tiers using the Karatsuba O(n^2.585) scaling model.\n");

    // -----------------------------------------------------------------------
    // Phase 3: Scaling projections from the largest measured tier.
    //          Three views: Karatsuba O(n^2.585), schoolbook O(n^3), empirical fit.
    // -----------------------------------------------------------------------
    if let Some((ref base_name, ref base_summary)) = measured.last() {
        // Compute empirical exponent from the two largest measured tiers
        let empirical_exp = if measured.len() >= 2 {
            let (_, ref prev) = measured[measured.len() - 2];
            let exp = scaling::empirical_exponent(
                prev.field_bits,
                prev.toffoli_gates,
                base_summary.field_bits,
                base_summary.toffoli_gates,
            );
            println!(
                "Empirical scaling exponent ({}-bit → {}-bit): {:.3}",
                prev.field_bits, base_summary.field_bits, exp,
            );
            Some(exp)
        } else {
            None
        };

        println!(
            "\n=== Scaling Projections (from {} baseline, Karatsuba O(n^2.585)) ===\n",
            base_name
        );
        let projections = scaling::project_scaling(
            base_summary.logical_qubits_peak,
            base_summary.toffoli_gates,
            base_summary.field_bits,
        );
        scaling::print_scaling_table(&projections);

        // Also show schoolbook O(n³) for comparison
        println!("\n=== Schoolbook O(n^3) Projections (for comparison) ===\n",);
        let schoolbook_projections = scaling::project_scaling_schoolbook(
            base_summary.logical_qubits_peak,
            base_summary.toffoli_gates,
            base_summary.field_bits,
        );
        scaling::print_scaling_table(&schoolbook_projections);

        // If we have an empirical fit, show that too
        if let Some(exp) = empirical_exp {
            println!("\n=== Empirical Fit O(n^{:.2}) Projections ===\n", exp,);
            let emp_projections = scaling::project_scaling_empirical(
                base_summary.logical_qubits_peak,
                base_summary.toffoli_gates,
                base_summary.field_bits,
                exp,
            );
            scaling::print_scaling_table(&emp_projections);
        }
        println!();

        // Extract 256-bit projection (Karatsuba model) for comparison table
        let projection_256 = projections
            .iter()
            .find(|p| p.field_bits == 256)
            .map(|p| (p.projected_qubits, p.projected_toffoli));

        comparison::print_comparison_table(projection_256);
        println!();
    } else {
        comparison::print_comparison_table(None);
        println!();
    }

    // -----------------------------------------------------------------------
    // Phase 4: Oath-N tier definitions and JSON summary.
    // -----------------------------------------------------------------------
    oath_tiers::print_oath_tiers();

    if let Some((_, ref summary)) = measured.last() {
        println!("\n=== JSON Circuit Summary (largest measured tier) ===\n");
        println!(
            "{}",
            serde_json::to_string_pretty(summary).unwrap_or_default()
        );
    }
}

fn run_export_qasm(output_path: Option<String>) {
    let project_dir = find_project_dir();
    let all_params_path = project_dir.join("sage/oath_all_params.json");

    // Default to Oath-16: small enough for fast export, large enough for
    // a meaningful gate sequence in the QASM output.
    let (tier_name, curve) = match params::load_all_curve_params(&all_params_path) {
        Ok(curves) => curves
            .into_iter()
            .find(|(n, _)| n == "Oath-16")
            .unwrap_or_else(|| {
                eprintln!("Oath-16 not found, falling back to hardcoded Oath-64 params");
                ("Oath-64".to_string(), hardcoded_oath64())
            }),
        Err(e) => {
            eprintln!("Warning: {}", e);
            eprintln!("Falling back to hardcoded Oath-64 parameters.\n");
            ("Oath-64".to_string(), hardcoded_oath64())
        }
    };

    let w = window_for_field(curve.field_bits);

    eprintln!(
        "Building {} Jacobian group-action circuit ({}-bit, window={})...",
        tier_name, curve.field_bits, w
    );
    let circuit = group_action_circuit::build_group_action_circuit_jacobian(&curve, w);

    let summary = circuit.summary();
    eprintln!(
        "  Qubits: {}, Toffoli: {}, Total gates: {}",
        summary.logical_qubits_peak, summary.toffoli_gates, summary.total_reversible_gates,
    );

    let qasm = export_qasm(&circuit);

    let default_name = format!(
        "oathbreaker_{}.qasm",
        tier_name.to_lowercase().replace('-', "")
    );
    let out = output_path.unwrap_or(default_name);
    match std::fs::write(&out, &qasm) {
        Ok(()) => {
            eprintln!("QASM written to: {}", out);
            eprintln!("  Lines: {}", qasm.lines().count());
            eprintln!("  Size:  {} bytes", qasm.len());
        }
        Err(e) => {
            eprintln!("Failed to write {}: {}", out, e);
            print!("{}", qasm);
        }
    }

    // Write JSON stats alongside
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

    for (tier_name, curve) in &all_curves {
        let w = window_for_field(curve.field_bits);
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
fn hardcoded_oath64() -> ec_goldilocks::CurveParams {
    use goldilocks_field::GoldilocksField;

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
    eprintln!("  export-qasm [FILE] Export circuit as OpenQASM 3.0");
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
