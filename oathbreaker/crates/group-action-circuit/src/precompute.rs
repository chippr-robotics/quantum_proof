use ec_goldilocks::curve::{AffinePoint, CurveParams};
use ec_goldilocks::point_ops::scalar_mul;

/// Precomputed table of curve points for windowed scalar multiplication.
///
/// For window size w, precomputes [0]P, [1]P, [2]P, ..., [2^w - 1]P
/// for an arbitrary base point P. These are baked into the circuit as
/// constants — they're parameter-dependent and known at construction time.
///
/// In the double-scalar formulation, two tables are needed:
/// - One for the generator G (known at curve generation time)
/// - One for the target point Q (known at proof time per instance)
#[derive(Clone, Debug)]
pub struct PrecomputeTable {
    /// The precomputed points: table[i] = [i]P.
    pub points: Vec<AffinePoint>,
    /// Window size that generated this table.
    pub window_size: usize,
    /// Label for debugging (e.g., "G" or "Q").
    pub label: String,
}

impl PrecomputeTable {
    /// Generate a precomputation table for a specific base point.
    pub fn generate_for_point(
        curve: &CurveParams,
        base_point: &AffinePoint,
        window_size: usize,
    ) -> Self {
        let table_size = 1usize << window_size;
        let mut points = Vec::with_capacity(table_size);

        for i in 0..table_size {
            let point = scalar_mul(i as u64, base_point, curve);
            points.push(point);
        }

        Self {
            points,
            window_size,
            label: String::new(),
        }
    }

    /// Generate a precomputation table for the curve generator G.
    pub fn generate_for_generator(curve: &CurveParams, window_size: usize) -> Self {
        let mut table = Self::generate_for_point(curve, &curve.generator, window_size);
        table.label = "G".to_string();
        table
    }

    /// Look up a point by scalar window value.
    pub fn lookup(&self, index: usize) -> &AffinePoint {
        &self.points[index]
    }

    /// Number of entries in the table.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Check if table is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}
