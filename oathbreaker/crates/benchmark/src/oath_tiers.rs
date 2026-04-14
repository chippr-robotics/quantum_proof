use serde::{Deserialize, Serialize};

/// Definition of Oath-N benchmark tiers.
///
/// Each tier represents an ECDLP instance at a specific difficulty level.
/// Quantum hardware is scored by the highest Oath-N level it can crack.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OathTier {
    /// Tier name (e.g., "Oath-8", "Oath-64").
    pub name: String,
    /// Field size in bits.
    pub field_bits: usize,
    /// Logical qubits (peak) for the Jacobian group-action circuit.
    /// Oath-8/16/32: measured from circuit construction.
    /// Oath-64: projected via linear qubit scaling.
    pub logical_qubits: usize,
    /// Toffoli gate count for the Jacobian group-action circuit.
    /// Oath-8/16/32: measured from circuit construction.
    /// Oath-64: projected via Karatsuba O(n^2.585) scaling.
    pub toffoli_gates: usize,
    /// Classical difficulty description.
    pub classical_difficulty: String,
    /// Target hardware era.
    pub target_era: String,
}

/// Return the defined Oath-N benchmark tiers.
///
/// Qubit and Toffoli counts from Jacobian projective coordinate circuit
/// (mixed addition, 0 per-op inversions, single final BGCD inversion,
/// Karatsuba multiplication, windowed scalar mul with one-hot QROM).
///
/// Oath-8/16/32: measured from actual circuit construction.
/// Oath-64: projected (circuit materialization exceeds CI memory at ~3 GB).
///
/// Qubit counts are peak logical qubits including exponent registers,
/// Jacobian accumulator (X,Y,Z), and all arithmetic ancillae (BGCD
/// workspace, Karatsuba workspace, QROM decode, intermediate registers).
pub fn oath_tiers() -> Vec<OathTier> {
    vec![
        OathTier {
            name: "Oath-8".to_string(),
            field_bits: 8,
            logical_qubits: 295,
            toffoli_gates: 162_000,
            classical_difficulty: "Trivial (by hand)".to_string(),
            target_era: "2026-2027".to_string(),
        },
        OathTier {
            name: "Oath-16".to_string(),
            field_bits: 16,
            logical_qubits: 855,
            toffoli_gates: 997_000,
            classical_difficulty: "Trivial (milliseconds)".to_string(),
            target_era: "2027-2028".to_string(),
        },
        OathTier {
            name: "Oath-32".to_string(),
            field_bits: 32,
            logical_qubits: 2_848,
            toffoli_gates: 5_760_000,
            classical_difficulty: "Easy (~seconds)".to_string(),
            target_era: "2029-2031".to_string(),
        },
        OathTier {
            name: "Oath-64".to_string(),
            field_bits: 64,
            logical_qubits: 5_696,
            toffoli_gates: 90_000_000,
            classical_difficulty: "Moderate (~hours via Pollard rho)".to_string(),
            target_era: "2032-2035".to_string(),
        },
    ]
}

/// Print the Oath-N benchmark tier table.
pub fn print_oath_tiers() {
    let tiers = oath_tiers();

    println!("=== The Oathbreaker Scale ===\n");
    println!("Score your quantum computer by which Oath curve it can crack.\n");
    println!(
        "┌──────────┬───────────┬──────────────┬──────────────┬──────────────────────────┬─────────────┐"
    );
    println!(
        "│ Tier     │ Field     │ Qubits       │ Toffoli      │ Classical Difficulty     │ Target Era  │"
    );
    println!(
        "├──────────┼───────────┼──────────────┼──────────────┼──────────────────────────┼─────────────┤"
    );
    for t in &tiers {
        println!(
            "│ {:<8} │ {:<9} │ {:<12} │ {:<12} │ {:<24} │ {:<11} │",
            t.name,
            format!("{}-bit", t.field_bits),
            t.logical_qubits,
            format_number(t.toffoli_gates),
            t.classical_difficulty,
            t.target_era,
        );
    }
    println!(
        "└──────────┴───────────┴──────────────┴──────────────┴──────────────────────────┴─────────────┘"
    );

    println!("\nScoring: Pass/fail. Did it recover the correct discrete log?");
    println!("No partial credit. Answer verified against classical oracle.");
}

fn format_number(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
