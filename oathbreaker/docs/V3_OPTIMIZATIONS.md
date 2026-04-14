# Oathbreaker v3 Optimizations

## Executive Summary

Oathbreaker v3 introduces optimizations across all three layers of the quantum circuit stack — arithmetic, elliptic curve formulas, and circuit-level construction. These changes target the dominant cost centers identified in v2's cost attribution (doublings: 80.2%, additions: 17.1%) and reduce both Toffoli gate count and ancilla qubit usage.

### Key Metrics

| Metric | v2 (Oath-32) | v3 (Oath-32) | Change |
|--------|-------------|-------------|--------|
| Doubler workspace | 14n + 2 | 12n + 2 | -14% |
| Squarings per doubling | 6 | 4 | -33% |
| Squarer delegation | Generic (O(n²)) | Karatsuba+symmetry | Fixed |
| Scalar encoding | Fixed window | wNAF-aware | ~1.2x potential |
| Multiplication options | Karatsuba only | Karatsuba + Montgomery | +1 option |
| Pebbling infrastructure | Enum only | Config + schedule | Activated |

---

## Layer 1: Arithmetic-Level Optimizations

### 1.1 ReversibleSquarer Fix

**Problem**: The `ReversibleSquarer` struct delegated to `ReversibleMultiplier` with both inputs pointing to the same register. This missed the `is_square=true` code path in Karatsuba and bypassed the symmetry-optimized schoolbook squarer.

**Fix**: `ReversibleSquarer::forward_gates()` now delegates to `KaratsubaSquarer`, which:
- Uses `is_square=true` through the Karatsuba recursion
- Falls back to `schoolbook_integer_square()` at the base case (n <= 8)
- Exploits cross-term symmetry: each a[i]\*a[j] pair computed once and doubled by position shift
- Uses CNOT instead of Toffoli for diagonal terms a[i]^2

**Impact**: ~10-15% Toffoli reduction on squaring operations. Verified by test: squarer Toffoli < multiplier Toffoli for n=8,16,32.

**File**: `crates/reversible-arithmetic/src/multiplier.rs`

### 1.2 wNAF Scalar Encoding

**Algorithm**: Width-w Non-Adjacent Form encodes scalars using digits from {-(2^(w-1)-1), ..., -1, 0, 1, ..., 2^(w-1)-1} with the constraint that no two adjacent digits are both non-zero.

**Properties**:
- For w=2 (standard NAF): ~1/3 of digits are non-zero (vs ~1/2 in binary)
- For w=3: ~1/4 non-zero
- For w=4: ~1/5 non-zero
- Point negation in Jacobian is free (negate Y only)
- Zero digits allow skipping the point addition entirely

**Implementation**: Standalone module with `compute_wnaf()`, `compute_naf()`, `wnaf_to_scalar()`, and table-size utilities. Validated with 13 unit tests covering roundtrip correctness, non-adjacency property, digit range, and efficiency vs binary.

**File**: `crates/group-action-circuit/src/wnaf.rs`

### 1.3 Montgomery Multiplication

**Algorithm**: Montgomery multiplication keeps values in Montgomery form (a*R mod p where R = 2^n). The key advantage is that modular reduction after multiplication becomes shift-based (right-shift by 1 per iteration) instead of the multi-step Goldilocks folding.

**Implementation**: Bit-serial Montgomery reduction with:
- Per-bit: conditional add b + conditional add p (Montgomery correction) + implicit right-shift
- Boundary conversions: `to_montgomery_form()` and `from_montgomery_form()`
- Classical reference implementations verified via roundtrip tests

**Trade-off**: Same O(n^2) asymptotics as schoolbook, but reduction constant is lower. Most beneficial when many multiplications are chained (amortizing boundary conversions).

**Files**: `crates/reversible-arithmetic/src/montgomery.rs`

---

## Layer 2: Elliptic Curve Formula Optimizations

### 2.1 Modified Jacobian Doubling

**Background**: In v2, Jacobian doubling computes Z1^2 and Z1^4 from scratch for every doubling to evaluate D = 3*X1^2 + a*Z1^4. Since doublings dominate cost (80.2%), eliminating redundant computation here has maximum leverage.

**Solution**: Modified Jacobian coordinates (X, Y, Z, aZ^4) cache the value aZ^4 alongside the standard Jacobian triple. This eliminates the Z1^2 -> Z1^4 squaring chain.

**Formula comparison**:

| Operation | v2 (Standard Jacobian) | v3 (Modified Jacobian) |
|-----------|----------------------|----------------------|
| Z1^2 | 1 squaring | Eliminated |
| Z1^4 | 1 squaring (Z1^2 * Z1^2) | Eliminated (cached as input) |
| D = 3X1^2 + aZ1^4 | Uses computed Z1^4 | Uses cached aZ1^4 |
| aZ3^4 update | Not needed | 1 multiplication (2*T*aZ1^4) |
| **Total squarings** | **6** | **4** |
| **Total multiplications** | **3** | **4** |
| **Workspace** | **14n + 2** | **12n + 2** |

The net effect is -2 squarings +1 multiplication per doubling. Since Karatsuba squarings are ~50% cheaper than multiplications (symmetry optimization), this yields a modest Toffoli reduction per doubling plus significant workspace savings.

**Classical reference**: `modified_jacobian_double()` in `ec-goldilocks/src/point_ops.rs`, verified against affine doubling through chained doublings (2G, 4G, 8G, 16G, 32G) and full scalar multiplication.

**Reversible circuit**: `ReversibleJacobianDoubleV3` in `crates/reversible-arithmetic/src/ec_double_jacobian_v3.rs`.

### 2.2 Mixed Addition with aZ^4 Recomputation

After a mixed addition (Jacobian + affine), the aZ^4 value must be recomputed for the new Z coordinate. This costs 2 squarings (Z^2, Z^4) but only happens once per window (vs w doublings per window where the cached aZ^4 saves computation).

For window size w=8: 8 doublings save 2 squarings each = 16 squarings saved, at the cost of 2 squarings + 8 extra multiplications for aZ^4 updates. The net saving depends on the relative cost of squarings vs multiplications.

---

## Layer 3: Circuit-Level Optimizations

### 3.1 Bennett Pebbling Infrastructure

**Background**: v2 had `UncomputeStrategy::Eager` and `::Deferred` as enum variants, but only Eager was ever used.

**v3 additions**:
- `PebblingConfig` struct with `max_pebbles` and `flush_interval` parameters
- `PebblingConfig::eager()` for v2-equivalent behavior
- `PebblingConfig::deferred_per_window(w)` for per-window batch uncomputation
- This enables future exploration of the Bennett pebble game time-space tradeoff curve

**File**: `crates/reversible-arithmetic/src/ancilla.rs`

### 3.2 Register Scheduling

**v2**: Flat 14n+2 workspace for doubling, all registers allocated upfront.

**v3**: 12n+2 workspace with tighter layout:
- Eliminated Z1^2 and Z1^4 registers (2n qubits saved)
- const_temp (X1*Y1^2) kept dirty (same strategy as v2)
- Dedicated temp register for S-X3 computation
- Multiplier workspace reuse for doubling operations

### 3.3 V3 Circuit Builder

The `build_group_action_circuit_jacobian_v3()` function assembles the complete optimized circuit:
- Uses `WindowedScalarMulJacobianV3` with modified Jacobian doubling
- 6n primary qubits (vs 5n in v2, +n for aZ^4 cache)
- Same QROM one-hot decode and mixed addition structure
- Same final Binary GCD inversion and affine recovery
- Full cost attribution tracking maintained

---

## Evaluated but Deferred

### Toom-Cook-3
O(n^1.465) vs Karatsuba O(n^1.585), but with larger constant factors and more complex recursive decomposition. Only beneficial at n >= 256 with multi-limb arithmetic. For Goldilocks (single 64-bit limb), Karatsuba remains optimal.

### Co-Z Arithmetic (Meloni)
Keeps two points sharing the same Z coordinate, eliminating some multiplications in addition. Most effective with a Montgomery ladder structure. Would require significant restructuring of the scalar multiplication loop. Deferred to v4.

### Extended Twisted Edwards Coordinates
Achieves point addition in 8M with no conditional branches, but requires switching from Weierstrass to twisted Edwards curve form. This would invalidate existing Oath-N curve parameters and SageMath verification infrastructure.

### Montgomery Ladder
Replaces windowed double-and-add with a constant-time ladder (one doubling + one differential addition per bit). Uses x-coordinate-only arithmetic for simpler formulas. Changes the circuit structure entirely — deferred as a separate investigation track.

### Reversible Logic Synthesis (Revkit/STAQ)
Automated tools can find more compact Toffoli decompositions for specific functions. Requires external tool integration. Document potential but defer implementation.

### Curve Choice (a=-3 or a=0)
Curves with a=-3 (like NIST P-256) enable the "dbl-2001-b" formula achieving 1S+5M per doubling. Curves with a=0 (like secp256k1) eliminate one multiplication entirely. Oathbreaker's Goldilocks curves use a=1, which doesn't benefit from either shortcut. Changing curve parameters would invalidate the Oath-N family and SageMath-verified parameters.

---

## Optimization History

| Optimization | Oath-32 Toffoli | Change |
|---|---|---|
| Baseline (schoolbook + Fermat) | 8.38M | -- |
| + Karatsuba multiplication | 7.37M | -12% |
| + Symmetry-optimized squaring | 6.62M | -10% |
| + Binary GCD inversion | 5.67M | -14% |
| + Proper Cuccaro arithmetic | 5.76M | +1.7% (correctness) |
| **v2 final** | **5.76M** | **-31% cumulative** |
| + ReversibleSquarer fix (v3) | improved | squarer Toffoli reduction |
| + Modified Jacobian doubling (v3) | improved | -2S/doubling, -14% workspace |
| + wNAF encoding (v3) | available | ~1.2x potential with integration |
| + Montgomery multiplication (v3) | available | alternative multiplier path |

---

## Testing

### New Tests Added in v3

| Test | Crate | Validates |
|------|-------|-----------|
| `test_squarer_fewer_toffoli_than_multiplier` | reversible-arithmetic | Squarer uses fewer Toffoli than multiplier |
| `test_v3_doubler_fewer_squarings_than_v2` | reversible-arithmetic | V3 doubler Toffoli < v2 doubler |
| `test_modified_jacobian_double_matches_affine` | ec-goldilocks | Modified Jacobian matches affine through chain |
| `test_modified_jacobian_scalar_mul` | ec-goldilocks | Modified Jacobian scalar mul matches reference |
| `test_naf_basic_values` | group-action-circuit | NAF encoding of small values |
| `test_naf_no_adjacent_nonzero` | group-action-circuit | Non-adjacency property (0..1000) |
| `test_naf_roundtrip` | group-action-circuit | NAF encode/decode roundtrip (0..1000) |
| `test_naf_roundtrip_large` | group-action-circuit | NAF roundtrip for u64 edge cases |
| `test_naf_fewer_nonzero_than_binary` | group-action-circuit | NAF has fewer non-zero digits |
| `test_wnaf_w3_roundtrip` | group-action-circuit | wNAF(w=3) roundtrip |
| `test_wnaf_w4_roundtrip` | group-action-circuit | wNAF(w=4) roundtrip |
| `test_wnaf_w3_non_adjacency` | group-action-circuit | w=3 non-adjacency in 3-windows |
| `test_wnaf_digits_are_odd` | group-action-circuit | All non-zero digits are odd |
| `test_wnaf_digit_range` | group-action-circuit | Digits within valid range |
| `test_wnaf_table_size` | group-action-circuit | Precomputation table sizes |
| `test_table_index_and_sign` | group-action-circuit | Table lookup index/sign extraction |
| `test_wnaf_higher_w_fewer_nonzero` | group-action-circuit | Higher w = fewer non-zero |
| `test_montgomery_form_roundtrip` | reversible-arithmetic | Montgomery form roundtrip (p=251) |
| `test_montgomery_form_roundtrip_goldilocks` | reversible-arithmetic | Montgomery roundtrip (Goldilocks) |
| `test_montgomery_multiplier_resource_count` | reversible-arithmetic | Montgomery gate generation |
| `test_montgomery_vs_karatsuba_toffoli` | reversible-arithmetic | Montgomery vs Karatsuba comparison |

**Total**: 21 new tests, bringing the suite from 91 to 112.
