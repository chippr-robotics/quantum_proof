use serde::{Deserialize, Serialize};

/// Scaling projection for larger field sizes based on measured 64-bit counts.
///
/// The circuit scales predictably with field size n:
/// - Qubits: O(n) — register widths scale linearly
/// - Toffoli gates per multiply: O(n²) — schoolbook
/// - Number of rounds: O(n) — scalar length
/// - Total Toffoli: O(n³) approximately
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScalingProjection {
    pub field_bits: usize,
    pub projected_qubits: usize,
    pub projected_toffoli: usize,
    pub label: String,
}

/// Project measured 64-bit resource counts to larger field sizes.
pub fn project_scaling(
    measured_qubits: usize,
    measured_toffoli: usize,
    base_bits: usize,
) -> Vec<ScalingProjection> {
    let mut targets: Vec<(usize, &str)> = Vec::new();

    // Include the measured baseline
    if base_bits <= 8 {
        targets.push((base_bits, "measured baseline"));
    }
    if base_bits <= 16 {
        targets.push((
            16,
            if base_bits == 16 {
                "Oath-16 (measured)"
            } else {
                "Oath-16 projection"
            },
        ));
    }
    if base_bits <= 32 {
        targets.push((
            32,
            if base_bits == 32 {
                "Oath-32 (measured)"
            } else {
                "Oath-32 projection"
            },
        ));
    }
    targets.push((
        64,
        if base_bits == 64 {
            "Oath-64 (measured)"
        } else {
            "Oath-64 projection"
        },
    ));
    targets.push((128, "Oath-128 projection"));
    targets.push((256, "Oath-256 / secp256k1"));
    targets.push((384, "Oath-384 / P-384"));
    targets.push((521, "Oath-521 / P-521"));

    // Deduplicate (base_bits may already be in the list)
    targets.dedup_by_key(|t| t.0);

    targets
        .into_iter()
        .map(|(bits, label)| {
            let ratio = bits as f64 / base_bits as f64;
            ScalingProjection {
                field_bits: bits,
                // Qubits scale as O(n)
                projected_qubits: if bits == base_bits {
                    measured_qubits
                } else {
                    (measured_qubits as f64 * ratio).ceil() as usize
                },
                // Toffoli scales as O(n³)
                projected_toffoli: if bits == base_bits {
                    measured_toffoli
                } else {
                    (measured_toffoli as f64 * ratio.powi(3)).ceil() as usize
                },
                label: label.to_string(),
            }
        })
        .collect()
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
