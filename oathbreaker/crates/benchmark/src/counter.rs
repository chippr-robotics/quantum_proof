use shor_circuit::ShorCircuit;

/// Aggregate gate and qubit statistics from a fully-constructed circuit.
pub fn print_resource_table(circuit: &ShorCircuit) {
    let summary = circuit.summary();

    println!("┌────────────────────────┬──────────────┐");
    println!("│ Metric                 │ Value        │");
    println!("├────────────────────────┼──────────────┤");
    println!(
        "│ Field size             │ {:<12} │",
        format!("{}-bit", summary.field_bits)
    );
    println!("│ Window size            │ {:<12} │", summary.window_size);
    println!(
        "│ Logical qubits (peak)  │ {:<12} │",
        summary.logical_qubits_peak
    );
    println!(
        "│ Toffoli gates          │ {:<12} │",
        summary.toffoli_gates
    );
    println!("│ CNOT gates             │ {:<12} │", summary.cnot_gates);
    println!("│ NOT gates              │ {:<12} │", summary.not_gates);
    println!(
        "│ Total reversible gates │ {:<12} │",
        summary.total_reversible_gates
    );
    println!(
        "│ Circuit depth          │ {:<12} │",
        summary.circuit_depth
    );
    println!(
        "│ Ancilla high-water     │ {:<12} │",
        summary.ancilla_high_water
    );
    println!(
        "│ QFT Hadamards          │ {:<12} │",
        summary.qft_hadamards
    );
    println!(
        "│ QFT rotations          │ {:<12} │",
        summary.qft_rotations
    );
    println!(
        "│ Point additions        │ {:<12} │",
        summary.point_additions
    );
    println!(
        "│ Point doublings        │ {:<12} │",
        summary.point_doublings
    );
    println!(
        "│ Field inversions       │ {:<12} │",
        summary.field_inversions
    );
    println!(
        "│ Field multiplications  │ {:<12} │",
        summary.field_multiplications
    );
    println!("└────────────────────────┴──────────────┘");
}
