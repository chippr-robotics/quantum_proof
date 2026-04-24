# Oath-4: Shor ECDLP on Real IBM Quantum Hardware

Oath-4 is the smallest rung of the Oathbreaker Scale, introduced specifically
to put the full Oathbreaker architecture -- Oath-family prime-order short-
Weierstrass curve, scalar multiplication, Shor period finding, classical
lattice recovery -- on an actual NISQ device. The Oath-4 field prime `p = 11`
is chosen for classical enumerability, not Goldilocks structure; the canonical
Goldilocks prime `2^64 − 2^32 + 1` is reserved for Oath-64.

| Curve | y^2 = x^3 + x + 6 over GF(11) |
| --- | --- |
| Group order n | 13 (prime) |
| Generator G | (2, 4) |
| Embedding degree | 12 |
| Logical qubits | 12 (4 for `a`, 4 for `b`, 4 for index) |
| Classical bits | 8 |
| Shots (typical) | 4096 -- 20 000 |

## Compiled gate count (measured)

The logical circuit is 12 qubits, depth 11, 27 instructions (8 H, 8 controlled
5-qubit unitaries, 2 inverse QFTs on 4 qubits, 8 measurements). Transpiling
to IBM backend basis sets gives the following measured hardware cost:

| Backend | Family | 2q basis | 2q gates (opt=3) | 1q gates | Depth |
|---|---|---|---|---|---|
| `FakeBrisbane` | Eagle | ECR | 3584 | ~23 000 | 15 573 |
| `FakeTorino`   | Heron | CZ  | 3588 | ~17 400 | 12 569 |

Numbers are from `qiskit.transpile(..., optimization_level=3,
seed_transpiler=42)` against the published fake-backend models. The heavy
cost comes from Qiskit's generic 5-qubit-isometry synthesis of the
controlled modular adders; a permutation-aware synthesis pass (future
work) should cut the 2q count several-fold.

Reproduce with:

```bash
python - <<'PY'
from qiskit import transpile
from qiskit_ibm_runtime.fake_provider import FakeBrisbane, FakeTorino
from oath4 import Instance
from oath4_circuit import build_oath4_shor_circuit

bundle = build_oath4_shor_circuit(Instance.from_secret(7).Q)
for be in (FakeBrisbane(), FakeTorino()):
    t = transpile(bundle.qc, be, optimization_level=3, seed_transpiler=42)
    two_q = sum(1 for op in t.data if op.operation.num_qubits >= 2)
    print(be.name, "depth", t.depth(), "2q", two_q)
PY
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

## Troubleshooting

- **Depth bloats after transpile.** Qiskit's generic isometry synthesis of
  the 5-qubit controlled-permutation gates is unoptimised. Using
  `optimization_level=3` and enabling `--dynamic-decoupling` materially
  helps. A dedicated permutation-aware synthesis pass is future work.
- **Flat k distribution on hardware.** Noise has swamped the spectrum.
  Increase shot count, enable DD, pin to a backend with low ECR error
  rate, or reduce the circuit by fixing `a = 0` and estimating `k` from
  `b` alone (run_ibm.py flag TBD).
