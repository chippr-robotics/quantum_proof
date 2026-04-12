use serde::{Deserialize, Serialize};

/// Scaling projection for larger field sizes based on measured counts.
///
/// The circuit scales predictably with field size n:
/// - Qubits: O(n) — register widths scale linearly
/// - Toffoli gates per multiply: O(n^1.585) with Karatsuba, O(n²) with schoolbook
/// - Number of rounds: O(n) — scalar length
/// - Total Toffoli: O(n^2.585) with Karatsuba, O(n³) with schoolbook
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScalingProjection {
    pub field_bits: usize,
    pub projected_qubits: usize,
    pub projected_toffoli: usize,
    pub label: String,
}

/// Standard target field sizes and labels.
fn build_targets(base_bits: usize) -> Vec<(usize, String)> {
    let mut targets: Vec<(usize, String)> = Vec::new();

    if base_bits <= 8 {
        targets.push((base_bits, "measured baseline".to_string()));
    }
    if base_bits <= 16 {
        targets.push((
            16,
            if base_bits == 16 {
                "Oath-16 (measured)".to_string()
            } else {
                "Oath-16 projection".to_string()
            },
        ));
    }
    if base_bits <= 32 {
        targets.push((
            32,
            if base_bits == 32 {
                "Oath-32 (measured)".to_string()
            } else {
                "Oath-32 projection".to_string()
            },
        ));
    }
    targets.push((
        64,
        if base_bits == 64 {
            "Oath-64 (measured)".to_string()
        } else {
            "Oath-64 projection".to_string()
        },
    ));
    targets.push((128, "Oath-128 projection".to_string()));
    targets.push((256, "Oath-256 / secp256k1".to_string()));
    targets.push((384, "Oath-384 / P-384".to_string()));
    targets.push((521, "Oath-521 / P-521".to_string()));

    targets.dedup_by_key(|t| t.0);
    targets
}

/// Project using a given Toffoli scaling exponent.
fn project_with_exponent(
    measured_qubits: usize,
    measured_toffoli: usize,
    base_bits: usize,
    toffoli_exponent: f64,
    targets: &[(usize, String)],
) -> Vec<ScalingProjection> {
    targets
        .iter()
        .map(|(bits, label)| {
            let ratio = *bits as f64 / base_bits as f64;
            ScalingProjection {
                field_bits: *bits,
                projected_qubits: if *bits == base_bits {
                    measured_qubits
                } else {
                    (measured_qubits as f64 * ratio).ceil() as usize
                },
                projected_toffoli: if *bits == base_bits {
                    measured_toffoli
                } else {
                    (measured_toffoli as f64 * ratio.powf(toffoli_exponent)).ceil() as usize
                },
                label: label.clone(),
            }
        })
        .collect()
}

/// Project measured resource counts using Karatsuba O(n^2.585) scaling.
///
/// This is the primary projection model now that the multiplier uses
/// Karatsuba decomposition (O(n^1.585) per multiply × O(n) rounds).
pub fn project_scaling(
    measured_qubits: usize,
    measured_toffoli: usize,
    base_bits: usize,
) -> Vec<ScalingProjection> {
    let targets = build_targets(base_bits);
    project_with_exponent(measured_qubits, measured_toffoli, base_bits, 2.585, &targets)
}

/// Project using the old O(n³) schoolbook scaling (for comparison).
pub fn project_scaling_schoolbook(
    measured_qubits: usize,
    measured_toffoli: usize,
    base_bits: usize,
) -> Vec<ScalingProjection> {
    let targets = build_targets(base_bits);
    project_with_exponent(measured_qubits, measured_toffoli, base_bits, 3.0, &targets)
}

/// Compute an empirical scaling exponent from two measured tiers.
///
/// Given (n1, t1) and (n2, t2), returns α such that t2/t1 ≈ (n2/n1)^α.
pub fn empirical_exponent(n1: usize, t1: usize, n2: usize, t2: usize) -> f64 {
    let ratio_n = n2 as f64 / n1 as f64;
    let ratio_t = t2 as f64 / t1 as f64;
    ratio_t.log2() / ratio_n.log2()
}

/// Project using an empirically fitted exponent from measured tiers.
pub fn project_scaling_empirical(
    measured_qubits: usize,
    measured_toffoli: usize,
    base_bits: usize,
    exponent: f64,
) -> Vec<ScalingProjection> {
    let targets = build_targets(base_bits);
    project_with_exponent(measured_qubits, measured_toffoli, base_bits, exponent, &targets)
}

/// Print the scaling projection table.
pub fn print_scaling_table(projections: &[ScalingProjection]) {
    println!("┌───────────┬──────────────────┬────────────────────┬─────────────────────────┐");
    println!("│ Field     │ Projected Qubits │ Projected Toffoli  │ Label                   │");
    println!("├───────────┼──────────────────┼────────────────────┼─────────────────────────┤");
    for p in projections {
        println!(
            "│ {:<9} │ {:<16} │ {:<18} │ {:<23} │",
            format!("{}-bit", p.field_bits),
            p.projected_qubits,
            p.projected_toffoli,
            p.label,
        );
    }
    println!("└───────────┴──────────────────┴────────────────────┴─────────────────────────┘");
}
