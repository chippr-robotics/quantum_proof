use ec_goldilocks::curve::{AffinePoint, CurveParams};
use goldilocks_field::GoldilocksField;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Raw JSON representation of curve parameters (matches sage output).
///
/// Note: The `order` field may exceed u64 for curves over the Goldilocks
/// field (the curve order ≈ p ± 2√p by Hasse's theorem, so for p ≈ 2^64
/// the order can exceed 2^64). We parse it as u128 and truncate to u64
/// for CurveParams, since circuit construction only uses field_bits.
#[derive(Deserialize)]
struct RawCurveParams {
    a: u64,
    b: u64,
    #[allow(dead_code)]
    p: u64,
    order: u128,
    generator_x: u64,
    generator_y: u64,
    field_bits: usize,
    #[allow(dead_code)]
    tier: String,
    #[allow(dead_code)]
    embedding_degree: Option<u128>,
    #[allow(dead_code)]
    discriminant: Option<u128>,
}

impl RawCurveParams {
    fn to_curve_params(&self) -> CurveParams {
        // The order may exceed u64 for 64-bit field curves. Circuit construction
        // only uses field_bits, so we truncate. The full order is only needed
        // for ECDLP solvers (Pollard rho, BSGS) which aren't used here.
        let order = if self.order <= u64::MAX as u128 {
            self.order as u64
        } else {
            // Store as much precision as u64 allows — flag in output
            eprintln!(
                "  Note: curve order {} exceeds u64; using truncated value for CurveParams",
                self.order,
            );
            (self.order % (u64::MAX as u128 + 1)) as u64
        };

        CurveParams {
            a: GoldilocksField::new(self.a),
            b: GoldilocksField::new(self.b),
            order,
            generator: AffinePoint::new(
                GoldilocksField::new(self.generator_x),
                GoldilocksField::new(self.generator_y),
            ),
            field_bits: self.field_bits,
        }
    }
}

/// Load a single curve's parameters from a JSON file (e.g., oath64_params.json).
pub fn load_curve_params(path: &Path) -> Result<CurveParams, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let raw: RawCurveParams = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;
    Ok(raw.to_curve_params())
}

/// Load all curve parameters from the combined oath_all_params.json file.
pub fn load_all_curve_params(path: &Path) -> Result<Vec<(String, CurveParams)>, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
    let raw_map: HashMap<String, RawCurveParams> = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))?;

    let mut params: Vec<(String, CurveParams)> = raw_map
        .into_iter()
        .map(|(name, raw)| (name, raw.to_curve_params()))
        .collect();

    // Sort by field_bits for consistent ordering
    params.sort_by_key(|(_, p)| p.field_bits);
    Ok(params)
}
