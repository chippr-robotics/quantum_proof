# Honest Implementation Roadmap

This document tracks the work required to lift the Oathbreaker
framework from a *resource-counting placeholder* (Goldilocks-prime
only, no `Q` ingestion, empty `gate_log`) to a *true implementation*
that emits a runnable Shor ECDLP circuit at any tier where the
classical and quantum sides agree on the field.

## What today's framework is, honestly

Three separate gaps make today's exported QASM a placeholder, not an
attack:

1. **Field arithmetic is hard-wired to the Goldilocks prime
   `p = 2^64 − 2^32 + 1`.** `oath-field/src/constants.rs:2` is the only
   modulus in the crate. The reversible multiplier
   (`reversible-arithmetic/src/multiplier.rs:127-235`) and adder
   (`reversible-arithmetic/src/adder.rs:133`) implement the
   Goldilocks-special-form reduction `2^64 ≡ 2^32 − 1 (mod p)`, which is
   not valid for any other prime. Even classical `point_add` in
   `ec-oath/src/point_ops.rs` operates over `GoldilocksField`, so the
   classical reference is wrong for any non-Goldilocks curve. The
   measured Oath-8/16/32 *resource numbers* are real (Karatsuba and BGCD
   gate counts don't depend on `p`), but the *gate semantics* are valid
   only at Oath-64.

2. **The target point `Q` never enters the circuit.** Both halves of
   the dual-scalar group action use the generator `G`'s precompute table
   (`scalar_mul.rs:101`, `scalar_mul_jacobian.rs:103`,
   `scalar_mul_jacobian_v3.rs:86`). The comment at
   `double_scalar.rs:127` acknowledges this: *"Q (target point) table
   would be generated at proof time with the specific instance."* The
   coherent map is therefore `[a]G + [b]G = [a+b]G`, which has no
   `k`-dependence and yields the trivial Shor relation `c1 = c2`.

3. **`gate_log` is never populated.** Every call site does
   `let _gates_a = scalar_mul_a.forward_gates(...)` — the returned
   `Vec<Gate>` is discarded and `gate_log: Vec::new()` is used to
   construct the `GroupActionCircuit`. The exported QASM contains the
   header, the qubit declaration, and effectively no gate body. The
   existing `qasm-export` CI only validates the header, so this has
   been silently broken.

The parallel Python POC at `oathbreaker/qiskit/poc/` works by exploiting
the cyclic-group isomorphism `E(F_p) ≅ Z/nZ` via `point_to_index`,
which is itself a discrete log by linear scan over the group. That is
a documented escape hatch that does not scale past Oath-16. It is a
NISQ-software-stack demonstration, not a cryptographic attack.

## What "honest" means for this branch

A run of the tier-N workflow must produce a circuit that, when
classically simulated on basis states, computes
`|a⟩|b⟩|0⟩ → |a⟩|b⟩|[a]G + [b]Q⟩` where:

- The circuit is constructed from `(curve, G, Q)` alone — no classical
  ECDLP oracle, no per-instance dlog precomputation.
- The classical precomputation budget is `O(2^w)` curve point
  operations per scalar mul (the Litinski/Google QROM-table recipe),
  scaling to 256-bit secp256k1 the same way it does to Oath-4.
- The reversible field arithmetic uses the curve's actual prime, with
  the modular reduction valid for that prime.
- Recovered `k` matches the planted `k` on the noiseless Aer simulator
  for every `k ∈ [1, n)`.

## Six-phase plan

### Phase 1 — Parameterised prime field

**Goal:** Introduce a generic prime-field type alongside
`GoldilocksField` without disturbing any existing Goldilocks-only call
site.

- New `oathbreaker/crates/oath-field/src/prime_field.rs`:
  - `PrimeField<const P: u64>` for compile-time-known primes (zero-cost,
    used for benchmarks at fixed primes).
  - `RuntimePrimeField` carrying the prime as runtime state (used for
    the per-tier curves loaded from JSON).
- Both implement: add, sub, neg, mul, square, pow, inverse (via
  Fermat for prime moduli), `from_canonical`, `to_canonical`.
- Tests: equivalence with `GoldilocksField` when `P = GOLDILOCKS_PRIME`;
  correctness on small primes (11, 251, 65521).

**Acceptance:** new tests pass; existing `cargo test -p oath-field`
unchanged.

### Phase 2 — Thread the curve prime through CurveParams + ec-oath

**Goal:** Make classical EC arithmetic correct for any prime, not just
Goldilocks.

- Extend `CurveParams` with `prime_modulus: u64` (the JSON already
  carries it as `p`; today it's discarded by `RawCurveParams::to_curve_params`).
- Refactor `point_add`, `point_double`, `scalar_mul`, etc. to use
  `RuntimePrimeField` instead of `GoldilocksField`.
- Keep a Goldilocks fast path (`scalar_mul_goldilocks`) for the
  zkVM/benchmark hot paths so we don't regress those.
- Tests: classical Oath-4 scalar mul recovers correct points (matches
  Sage / the Python `oath4.py` reference).

**Acceptance:** classical `[k]G` for the Oath-4 curve agrees with
`oathbreaker/qiskit/poc/oath4.py:ec_mul` for all `k ∈ [1, 13)`.

### Phase 3 — Generic reversible arithmetic

**Goal:** Replace Goldilocks-special-form reductions with a
prime-generic compare-and-subtract reduction.

- New module `reversible-arithmetic/src/mod_reduce.rs` — a reversible
  compare-and-conditional-subtract gadget for arbitrary `p < 2^n`.
- Refactor `multiplier.rs` to call `mod_reduce::reduce_mod_p` after
  the unreduced product. Keep the Goldilocks reduction as a fast
  `reduce_mod_goldilocks` for the existing Oath-64 path.
- Same treatment for `adder.rs`.
- Validate `BinaryGcdInverter` against arbitrary primes (the algorithm
  is prime-generic; only the workspace size assumptions need review).
- Tests: classical simulation of the gate sequences over basis states
  agrees with the corresponding `RuntimePrimeField` arithmetic.

**Acceptance:** new prop-test (≥256 cases) confirming `mul_mod_p`,
`add_mod_p`, `inverse_mod_p` over GF(11), GF(251), GF(65521) match
their classical counterparts when the gate sequence is simulated on
basis states.

### Phase 4 — Persist gate_log + thread target_q

**Goal:** The `GroupActionCircuit` struct actually contains the gates
it claims to compute, and the second scalar-mul half ingests `Q`.

- Collect the `Vec<Gate>` returned from every `forward_gates` call into
  `gate_log` in all three `build_group_action_circuit*` functions
  (affine v1, Jacobian v2, Modified-Jacobian v3). Don't forget the
  inline CNOT swaps between phases and the gates from the BGCD inverter
  + Karatsuba multipliers in the affine recovery step.
- Add `base_point: &AffinePoint` parameter to each scalar mul's
  `forward_gates`. Update the precompute table call to take it.
- Add a `target_q: &AffinePoint` parameter to each
  `build_group_action_circuit_*` builder. Pass `&curve.generator` to
  the first half, `target_q` to the second half. Existing 2-arg
  builders become wrappers that pass `target_q = &curve.generator` so
  current call sites and tests keep working.

**Acceptance:** the existing benchmark CI still reports identical
resource numbers; exported QASM at any tier is now nontrivial in size
and contains a number of gates equal to `toffoli + cnot + not`.

### Phase 5 — End-to-end Oath-4 correctness

**Goal:** Prove the assembled pipeline computes the right thing on the
Oath-4 curve.

- Add Oath-4 to `sage/oath_all_params.json`.
- Add an integration test
  `crates/group-action-circuit/tests/oath4_attack.rs` that:
  - Builds the Oath-4 attack circuit for `k = 7` (Q = [7]G on the
    Oath-4 curve over GF(11)).
  - Simulates the gate sequence classically on each basis-state input
    `(a, b)` for a small set of `(a, b)` pairs.
  - Asserts the index register holds `[a]G + [b]Q` after the gate
    sequence runs.
- Add a Shor-recovery test that uses
  `sample_measurement_pairs(7, 13, 64)` and asserts the post-processor
  recovers `k = 7`.

**Acceptance:** both integration tests pass.

### Phase 6 — Export + workflow integration + docs

**Goal:** The hardware deployment workflow can dispatch a true Oath-4
attack circuit.

- New CLI subcommand
  `cargo run -p benchmark -- export-attack-qasm --tier T --k K` that
  emits a Q-specific full Shor circuit via `export_shor_qasm`.
- Update `.github/workflows/quantum-deploy.yml` to call the new
  subcommand for tier=4 and route the resulting QASM through
  `oathN_hardware_runner.py`.
- Update `oathbreaker/docs/HARDWARE_DEPLOYMENT.md` to describe the
  honest path and retire the cyclic-group-shortcut POC default.
- Update `oathbreaker/qiskit/README.md` to point at the honest path.

**Acceptance:** workflow run on a fork with `IBM_QUANTUM_API` set
produces an artifact whose `manifest.json` has `recovered_k = planted_k`
when run on AerSimulator (post-Phase-5 unit test passes), and a
sensible transpile footprint when run on Heron in dry-run mode.

## Risk + scope

- Total scope: estimated 4–8 days of careful Rust work plus thin
  Python/YAML at the end. Most of the risk concentrates in Phase 3
  (rewriting the reduction in the multiplier without breaking the
  resource numbers existing tests assert) and Phase 5 (the gate-level
  classical simulator needed to verify correctness).
- Phases 1–2 are mostly additive and safe.
- Phase 4 is a refactor; tests are the safety net.
- Phase 5 is the moment of truth — if the integration test fails, the
  primitive bugs need to be hunted down before claiming a true
  implementation.

## Status (commit-by-commit, updated as work lands)

- [x] **Phase 1**: prime field module + tests
  (commit `45388f3`: `oath-field/src/prime_field.rs`, 28 tests)
- [x] **Phase 2**: CurveParams + ec-oath classical ops
  (commit `b495ddd`: `CurveParams::prime_modulus`, `point_ops_generic`,
  Oath-4 reference table verified, 127 workspace tests green)
- [ ] **Phase 3**: generic reversible reduction
- [ ] **Phase 4**: gate_log persistence + Q threading
- [ ] **Phase 5**: Oath-4 integration test
- [ ] **Phase 6**: CLI + workflow + docs

### Phase 3 design notes (handoff for the next session)

The Goldilocks reduction at `multiplier.rs:127-235` exploits
`2^64 ≡ 2^32 - 1 (mod p)` to fold the upper half of the unreduced
product back into the lower half via a fixed pattern of CNOT + Toffoli
chains. For arbitrary `p < 2^n`, the standard replacement is
**compare-and-conditional-subtract**:

1. Allocate one ancilla `cmp_flag`.
2. Use a reversible comparator (e.g. CuccaroAdder fed `2^n - p` as a
   classical constant) to set `cmp_flag = 1` iff `acc >= p`.
3. Conditionally subtract `p` from `acc`, controlled on `cmp_flag`.
4. Uncompute `cmp_flag` (it is now provably 0 since `acc < p`).

For the post-multiplication case where `acc` is `2n` bits, this fold
runs `n` times (once per high bit), or once on the entire low half
if intermediate folds are pre-summed. The Goldilocks fast path stays
in place for Oath-64; the generic path is selected when
`curve.prime_modulus != GOLDILOCKS_PRIME`.

Suggested module skeleton:

```text
reversible-arithmetic/src/mod_reduce.rs
    pub fn compare_ge(reg, k_bits, p, ws, counter) -> Vec<Gate>
        // sets ws[flag] = 1 iff (reg as integer) >= p
    pub fn subtract_const_controlled(reg, n_bits, p, ctrl, ws, counter)
        -> Vec<Gate>
        // reg -= p when ctrl is 1
    pub fn reduce_below_2p(reg[0..=n], p, ws, counter) -> Vec<Gate>
        // composes the above; assumes reg is in [0, 2p)
    pub fn reduce_after_multiplication(acc[0..2n], p, ws, counter)
        -> Vec<Gate>
        // generic version of the Goldilocks fold loop
```

Each function gets a property test that runs `Gate::apply` on random
basis-state inputs and asserts the post-state matches `value % p`.
This is the validation oracle that the existing tests never had.

Once Phase 3 lands, Phases 4-6 are mechanical (refactor scalar muls
to take `&AffinePoint`, persist `gate_log`, add the CLI subcommand,
update the workflow). Estimated 2-4 days for Phase 3 done carefully,
then ~1 day for Phases 4-6.

This branch must NOT be merged until Phase 5 passes. Until then it is
foundation work and the existing placeholder workflow on
`claude/quantum-deployment-action-7sOm0` remains the honest description
of what the deployment actually does today.
