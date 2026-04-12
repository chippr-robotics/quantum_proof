fn main() {
    // When the `sp1` feature is enabled, compile the guest program
    // (sp1-program) to a RISC-V ELF binary using the SP1 toolchain.
    // The resulting ELF is embedded into the host binary at compile time.
    #[cfg(feature = "sp1")]
    {
        sp1_build::build_program("../sp1-program");
    }
}
