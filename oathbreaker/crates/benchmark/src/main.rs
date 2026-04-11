mod comparison;
mod counter;
mod scaling;

fn main() {
    println!("=== Oathbreaker Benchmark Suite ===\n");

    // TODO: Build the circuit with real parameters and measure resources.
    //
    // let curve = load_curve_params("curve_params.json");
    // let circuit = shor_circuit::build_shor_circuit(&curve, 8);
    // counter::print_resource_table(&circuit);
    //
    // let projections = scaling::project_scaling(
    //     circuit.qubit_count(),
    //     circuit.toffoli_count(),
    //     64,
    // );
    // scaling::print_scaling_table(&projections);
    //
    // let projection_256 = projections.iter()
    //     .find(|p| p.field_bits == 256)
    //     .map(|p| (p.projected_qubits, p.projected_toffoli));
    // comparison::print_comparison_table(projection_256);

    println!("Benchmark suite stub — awaiting circuit implementation.\n");
    println!("Once the reversible arithmetic is complete:");
    println!("  cargo run --release -p benchmark");

    // For now, print the comparison table with prior work
    println!();
    comparison::print_comparison_table(None);
}
