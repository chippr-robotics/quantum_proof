use ec_goldilocks::curve::{AffinePoint, CurveParams};
use ec_goldilocks::point_ops::scalar_mul;

/// Precomputed table of curve points for windowed scalar multiplication.
///
/// For window size w, precomputes [0]G, [1]G, [2]G, ..., [2^w - 1]G.
/// These are baked into the circuit as constants — they're curve-parameter
/// dependent and known at circuit construction time.
#[derive(Clone, Debug)]
pub struct PrecomputeTable {
    /// The precomputed points: table[i] = [i]G.
    pub points: Vec<AffinePoint>,
    /// Window size that generated this table.
    pub window_size: usize,
}

impl PrecomputeTable {
    /// Generate the precomputation table for the given curve and window size.
    pub fn generate(curve: &CurveParams, window_size: usize) -> Self {
        let table_size = 1usize << window_size;
        let mut points = Vec::with_capacity(table_size);

        for i in 0..table_size {
            let point = scalar_mul(i as u64, &curve.generator, curve);
            points.push(point);
        }

        Self {
            points,
            window_size,
        }
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
