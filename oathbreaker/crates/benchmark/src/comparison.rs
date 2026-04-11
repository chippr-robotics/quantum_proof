use serde::{Deserialize, Serialize};

/// Published resource estimates from prior work for comparison.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorWork {
    pub author: String,
    pub year: u16,
    pub field_bits: usize,
    pub qubits: Option<usize>,
    pub toffoli: Option<usize>,
    pub notes: String,
}

/// Reference numbers from published literature.
pub fn prior_work_table() -> Vec<PriorWork> {
    vec![
        PriorWork {
            author: "Roetteler et al.".to_string(),
            year: 2017,
            field_bits: 256,
            qubits: Some(2330),
            toffoli: Some(448_000_000_000),
            notes: "ASIACRYPT 2017, first detailed ECDLP estimates".to_string(),
        },
        PriorWork {
            author: "Häner et al.".to_string(),
            year: 2020,
            field_bits: 256,
            qubits: Some(2048),
            toffoli: Some(126_000_000_000),
            notes: "Improved circuits for ECDLP".to_string(),
        },
        PriorWork {
            author: "Litinski".to_string(),
            year: 2023,
            field_bits: 256,
            qubits: None,
            toffoli: Some(50_000_000),
            notes: "50M Toffoli via windowed arithmetic + measurement-based uncomp".to_string(),
        },
        PriorWork {
            author: "Chevignard, Fouque, Schrottenloher".to_string(),
            year: 2026,
            field_bits: 256,
            qubits: None,
            toffoli: None,
            notes: "EUROCRYPT 2026 — INRIA improved circuits".to_string(),
        },
        PriorWork {
            author: "Babbush, Zalcman, Gidney et al. (Google)".to_string(),
            year: 2026,
            field_bits: 256,
            qubits: None,
            toffoli: None,
            notes: "March 2026 — ZK proof of point addition only, full circuit withheld"
                .to_string(),
        },
    ]
}

/// Print the comparison table.
pub fn print_comparison_table(our_projection_256: Option<(usize, usize)>) {
    let prior = prior_work_table();

    println!("=== Comparison to Prior Work (256-bit ECDLP) ===\n");
    println!(
        "┌──────────────────────────────────────┬──────┬─────────┬──────────────┬──────────────────────────┐"
    );
    println!(
        "│ Author                               │ Year │ Qubits  │ Toffoli      │ Notes                    │"
    );
    println!(
        "├──────────────────────────────────────┼──────┼─────────┼──────────────┼──────────────────────────┤"
    );
    for p in &prior {
        println!(
            "│ {:<36} │ {:<4} │ {:<7} │ {:<12} │ {:<24} │",
            p.author,
            p.year,
            p.qubits
                .map(|q| q.to_string())
                .unwrap_or_else(|| "N/A".to_string()),
            p.toffoli
                .map(|t| format_large_number(t))
                .unwrap_or_else(|| "N/A".to_string()),
            truncate(&p.notes, 24),
        );
    }

    if let Some((qubits, toffoli)) = our_projection_256 {
        println!(
            "│ {:<36} │ {:<4} │ {:<7} │ {:<12} │ {:<24} │",
            "Oathbreaker (projected)",
            2026,
            qubits,
            format_large_number(toffoli),
            "Oath-64→256 projection",
        );
    }

    println!(
        "└──────────────────────────────────────┴──────┴─────────┴──────────────┴──────────────────────────┘"
    );
}

fn format_large_number(n: usize) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
