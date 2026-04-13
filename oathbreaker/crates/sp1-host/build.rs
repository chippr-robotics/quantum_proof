fn main() {
    // When the `sp1` feature is enabled, compile the guest program
    // (sp1-program) to a RISC-V ELF binary using the SP1 toolchain.
    // The resulting ELF is embedded into the host binary at compile time.
    //
    // We pass --features sp1 so the guest activates #![no_main] and
    // the sp1_zkvm::entrypoint! macro (without it, the guest compiles
    // as a regular binary without the SP1 entry point).
    #[cfg(feature = "sp1")]
    {
        let args = sp1_build::BuildArgs {
            features: vec!["sp1".to_string()],
            ..Default::default()
        };
        sp1_build::build_program_with_args("../sp1-program", args);
    }
}
