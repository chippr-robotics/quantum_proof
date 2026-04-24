# Oath-4 / Oath-N Proof of Concept

> **This directory is the Oathbreaker NISQ proof of concept.** Every
> circuit here compiles the target ECDLP instance by classically
> pre-solving it: the circuit for `Q = [k]G` uses `point_to_index` at
> construction time, which is equivalent to discrete log. This is a
> genuine Shor *protocol* run but **not** a cryptographic attack.
>
> It exists to validate the Qiskit / IBM software stack (transpile,
> SamplerV2, dynamic circuits, error mitigation, Shor post-processing)
> at sizes that fit today's hardware.
>
> The *real* Oath-N path -- no classical ECDLP oracle anywhere -- lives
> in the parent directory [`../`](../). The contract between the two
> paths is written down in
> [`../../docs/NISQ_ROADMAP.md`](../../docs/NISQ_ROADMAP.md).

Oath-4 is the smallest rung of the Oathbreaker Scale, introduced specifically
to put the full Oathbreaker architecture -- Oath-family prime-order short-
Weierstrass curve, scalar multiplication, Shor period finding, classical
lattice recovery -- on an actual NISQ device. The Oath-4 field prime `p = 11`
is chosen for classical enumerability, not Goldilocks structure; the canonical
Goldilocks prime `2^64 − 2^32 + 1` is reserved for Oath-64.

## State of the art (as of April 2026)

Project Eleven's Q-Day Prize was awarded to Giancarlo Lelli for breaking a
**15-bit ECC key** on publicly accessible NISQ hardware -- a 512x jump over
Tippeconnic's 6-bit demonstration in September 2025. Lelli's 15-bit result
sits between our Oath-8 and Oath-16 tiers, and was achieved with a "variant
of Shor's algorithm" (almost certainly iterative phase estimation + dynamic
circuits + heavy custom synthesis / mitigation). The roadmap below matches
the techniques that would have to be in any such demonstration.

| Curve | y^2 = x^3 + x + 6 over GF(11) |
| --- | --- |
| Group order n | 13 (prime) |
| Generator G | (2, 4) |
| Embedding degree | 12 |
| Logical qubits | 12 (4 for `a`, 4 for `b`, 4 for index) |
| Classical bits | 8 |
| Shots (typical) | 4096 -- 20 000 |

## Configurable Oath-N builder

The modules split cleanly across:

| Module | Role |
| --- | --- |
| `oath_curve.py` | Generic ``OathCurve.load_tier(N)`` + EC arithmetic for any Oath tier |
| `modular_adders.py` | Pluggable controlled-add-const-mod-N registry (currently `qft_beauregard` and `cdkm_ripple`) |
| `oathN_circuit.py` | Generic Shor ECDLP builder: ``build_oathN_shor_circuit(curve, Q, adder_method=...)`` |
| `oath4_optimized.py` | Oath-4 convenience wrapper that delegates to the generic builder |
| `oath4_circuit.py` | Baseline 12-qubit Oath-4 circuit (kept for comparison) |

Usage:

```python
from oath_curve import OathCurve, OathInstance
from oathN_circuit import build_oathN_shor_circuit

curve = OathCurve.load_tier(8)                 # 8, 16, 32, ...
inst = OathInstance.from_secret(curve, 42)
bundle = build_oathN_shor_circuit(
    curve, inst.Q, adder_method="cdkm_ripple",
)
```

The NISQ compilation path (cyclic-group isomorphism `E ~= Z/nZ`) requires a
classical scan to resolve each controlled-add-point into an integer
constant, so it is tractable up through Oath-16. For Oath-32 and above
the real route is the full reversible EC-arithmetic circuit built by the
Rust `crates/group-action-circuit` crate.

## Compiled gate count (measured, FakeTorino / Heron CZ basis)

`qiskit.transpile(..., optimization_level=3, seed_transpiler=42)` on the
published fake-backend models:

| Tier | Method | Qubits | 2q | Depth |
|---|---|---:|---:|---:|
| Oath-4  | baseline (UnitaryGate.ctrl) | 12 | 3 589 | 12 569 |
| Oath-4  | qft_beauregard | 14 | **2 811** | **5 948** |
| Oath-4  | cdkm_ripple | 20 | 8 127 | 20 037 |
| Oath-8  | qft_beauregard | 29 | **28 348** | 44 729 |
| Oath-8  | cdkm_ripple | 40 | 38 349 | 91 808 |
| Oath-16 | qft_beauregard | 53 | 186 426 | **202 433** |
| Oath-16 | cdkm_ripple | 72 | **134 865** | 320 442 |

**Scaling crossover.** At Oath-4 the QFT-Beauregard adder is 3x smaller
than CDKM: the O(n^2) QFT wrappers are cheap when n is tiny. At Oath-16
the crossover has arrived -- CDKM is 28% fewer 2q gates than Beauregard,
matching its theoretical O(n) per-add scaling. Depth is still higher for
CDKM because the Beauregard wrapper (5 sub-adds per controlled add) is
depth-dominant; fusing wrappers and moving to semiclassical IQPE are the
next-up items in the roadmap below.

**NISQ feasibility.** Heron's budget is ~4 400 CZ-steps of depth and
well under 1 error event per shot. Oath-4 / qft_beauregard fits T2 for
the first time; Oath-8 and Oath-16 are still well over budget. Lelli's
15-bit result shows the envelope is reachable, but only with the stack
(IQPE + approximate QFT + permutation-aware synthesis + error
mitigation) layered on top of what this PR lands.

Reproduce with:

```bash
python measure_gate_count.py --tiers 4,8,16 --backends torino
```

## Why Oath-4

The Oathbreaker framework demonstrates a full Shor ECDLP stack at Oath-8,
Oath-16, and Oath-32 tiers -- but those tiers require thousands of logical
qubits and millions of Toffoli gates. Oath-4 closes the gap to today's
hardware:

1. **End-to-end architectural validation.** The same reversible group-
   action primitive that Oath-32 builds out of Karatsuba multipliers and
   Binary-GCD inverters is here compiled, via the cyclic-group isomorphism
   `E(F_p) ~= Z/nZ`, into eight controlled modular-addition permutations on
   a 4-qubit index register. The physical run corroborates the classical
   zkVM proof that the Oath circuit is functionally correct.

2. **NISQ software stack verification.** Transpilation, dynamic decoupling,
   sampler pipelines, SamplerV2 post-selection -- every layer between the
   logical circuit and the pulse level is exercised. A working Oath-4 run
   is evidence that the NISQ toolchain can be driven by the Oathbreaker
   framework all the way to silicon.

3. **A novel benchmark floor.** Extends the Oathbreaker Scale below the
   previously NISQ-unreachable Oath-8 tier, giving hardware teams a genuine
   ECDLP instance that fits inside current coherence budgets.

## Files

| File | Role |
| --- | --- |
| `oath4.py` | Curve definition, point arithmetic, classical dlog oracle |
| `oath4_circuit.py` | Qiskit circuit builder and classical post-processor |
| `run_simulator.py` | Noiseless AerSimulator pre-flight |
| `run_ibm.py` | Real hardware runner (IBM Runtime SamplerV2) |
| `test_oath4.py` | Sweep all 12 non-zero secrets through the simulator |
| `requirements.txt` | pinned dep set |

## Quick start

```bash
pip install -r requirements.txt

# Classical sanity check.
python oath4.py

# Noiseless simulator (should recover every secret k=1..12 at ~70% peak).
python test_oath4.py

# Single hardware run.
export IBM_QUANTUM_TOKEN=...    # from https://quantum.ibm.com/
python run_ibm.py --k 7 --shots 20000 --dynamic-decoupling
```

## Circuit recipe

For an instance `Q = [k]G` on the Oath-4 curve:

1. Create two exponent registers `a` (4 qubits) and `b` (4 qubits),
   Hadamard-initialised to the uniform superposition of 0..15.
2. Initialise a 4-qubit index register `idx` to `|0> = |infinity>`.
3. For each `j in 0..3`, apply:
   - controlled on `a[j]`, the permutation `|idx> -> |idx + [2^j]G>`
   - controlled on `b[j]`, the permutation `|idx> -> |idx + [2^j]Q>`
   Both permutations are built from the classical EC addition law --
   the `Q`-permutations depend on `Q` only, not on the secret `k`.
4. Apply an inverse QFT to `a` and `b` *independently* (a joint IQFT
   would scramble the bilinear Shor spectrum).
5. Measure both exponent registers.

## Classical post-processing

Each shot yields a pair of 4-bit integers `(c1, c2)`. With `N = 16` and
`n = 13`,

```
d1 = round(c1 * n / N) mod n
d2 = round(c2 * n / N) mod n
k  = d2 * modinv(d1, n) mod n
```

Take the mode across shots. The noiseless simulator gives ~70% correct
peak; NISQ hardware at present typically gives well above the naive 1/12
random baseline with DD + a few thousand shots, enough for clear voting.

## Verification flow

| Stage | Artefact |
| --- | --- |
| Classical DLP oracle | `classical_dlog(Q)` in `oath4.py` |
| Reversible circuit proof | SP1 + Groth16 (see `../crates/sp1-host`) |
| Noiseless quantum sim | `test_oath4.py` (sweep all 12 secrets) |
| IBM hardware | `run_ibm.py` against Eagle / Heron backends |

A passing score on a given machine is defined as: given a published `Q`,
the machine returns the unique `k in 1..12` with `[k]G = Q`, and the
correct-vote majority can be verified against the classical oracle in
microseconds.

## Roadmap to Oath-8 / Oath-16 NISQ

Lelli's 15-bit demonstration is a forcing function for scaling our
architecture beyond Oath-4. Naively projecting our current optimized
(Beauregard QFT) circuit:

| Tier | Logical qubits | Projected 2q on Heron | Projected depth |
|---:|---:|---:|---:|
| Oath-4 (measured) | 14 | 2 811 | 5 948 |
| Oath-8 (projected) | 26 | ~15 000 | ~1 900 |
| Oath-16 (projected) | 50 | ~99 000 | ~6 200 |

The Oath-16 cost grows as `O(n^3)` because each Beauregard call carries
`O(n^2)` QFT overhead and we do `O(n)` of them. Oath-16 is roughly 35x
over the Heron 2q-error budget and ~1.5x over T2. Closing this gap, in
priority order:

1. **Pluggable modular adder** -- factor `controlled_add_const_mod_n`
   into a `method=` interface so we can drop in alternatives without
   rewriting the Shor builder. (Refactor in `oath4_optimized.py`.)
2. **CDKM ripple-carry add** for the controlled-add-constant. Trades
   the QFT wrappers for a classical reversible adder; for n=16 this is
   about a 10x reduction in 2q gates per add.
3. **Semiclassical IQPE** -- single counting qubit + mid-circuit
   measurement + classical feedforward via `qiskit.circuit.IfElseOp`.
   Removes one of the two exponent registers and halves the controlled-
   add count. Targets dynamic-circuit-capable backends (most of IBM's
   current fleet).
4. **Approximate QFT** for the readout step -- prune controlled-phases
   below `~ epsilon / 2*pi` for a tolerance `epsilon`; cuts depth on
   the IQFT by a factor of `n / log(n)`.
5. **Permutation-aware synthesis pass** -- recognise our controlled
   modular adders as basis-state permutations rather than generic
   isometries; saves another 2-3x.
6. **Error mitigation** layer (ZNE / PEC / DD / twirling). Buys back
   roughly an order of magnitude of effective fidelity at the cost of
   ~10x more shots.

Stacked, the optimistic envelope for Oath-16 is ~3 000 CZ on Heron --
comparable to today's Oath-4 baseline. That's roughly the budget any
"15-bit ECC on NISQ" demonstration has to fit inside.

## Troubleshooting

- **Depth bloats after transpile.** Qiskit's generic isometry synthesis of
  the 5-qubit controlled-permutation gates is unoptimised. Using
  `optimization_level=3` and enabling `--dynamic-decoupling` materially
  helps. A dedicated permutation-aware synthesis pass is future work.
- **Flat k distribution on hardware.** Noise has swamped the spectrum.
  Increase shot count, enable DD, pin to a backend with low ECR error
  rate, or reduce the circuit by fixing `a = 0` and estimating `k` from
  `b` alone (run_ibm.py flag TBD).
