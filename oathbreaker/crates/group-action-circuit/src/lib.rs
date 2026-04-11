pub mod double_scalar;
pub mod export;
pub mod precompute;
pub mod qft_stub;
pub mod scalar_mul;
pub mod scalar_mul_jacobian;

#[cfg(test)]
mod tests;

pub use double_scalar::{
    build_group_action_circuit, build_group_action_circuit_jacobian,
    CircuitSummary, GroupActionCircuit,
};
