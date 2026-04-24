// Re-export param loading from ec-goldilocks to avoid duplication.
// Both the benchmark crate and sp1-host use these functions.

pub use ec_oath::params::load_all_curve_params;
