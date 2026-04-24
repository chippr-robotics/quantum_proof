# Oathbreaker Qiskit Stack

This directory holds the Qiskit / IBM Quantum integration for the
Oathbreaker ECDLP framework. It is organised into **two deliberately
separate paths**:

| Path | Directory | Classical precomputation? | Scales to crypto sizes? |
| --- | --- | --- | --- |
| **Proof of concept (POC)** | [`poc/`](poc/) | **Yes** (compile-time dlog lookup) | **No** (only as far as the classical dlog is tractable) |
| **Real Oath-N** | this directory | **No** (zero classical ECDLP used) | Yes, once the resource count fits the hardware era |

## The distinction, plainly

The POC path achieves small compiled circuits by compiling each
controlled-add-point operation into a controlled-add-**constant**-mod-n via
the cyclic-group isomorphism `E(F_p) ~= Z/nZ`. Computing that "constant"
requires knowing the discrete log of `[2^j]Q` for every bit `j`, which the
POC does **classically at circuit-construction time** via linear scan over
the group. For Oath-4/8/16 the scan is trivial; past Oath-16 the classical
dlog is itself the thing Shor is supposed to solve, so the shortcut
collapses.

This makes the POC useful for two things:

1. **Validating the NISQ software stack.** Transpile / SamplerV2 /
   dynamic circuits / DD / error mitigation all get exercised against
   real hardware at Oath-4 sizes.
2. **Characterising the compiled-gate budget** for circuits with the same
   *shape* as a real Shor ECDLP attack (two controlled-add registers,
   joint IQFT, index register).

It does **not** constitute a cryptographic attack. A circuit whose
construction depends on pre-solving the ECDLP it claims to attack offers
no quantum advantage over just printing the answer.

## Real Oath-N path (this directory)

Everything here is subject to the following rule:

> **No classical ECDLP oracle at any stage of circuit construction.** The
> quantum circuit must be built from the curve equation and the public
> point `Q` alone. The only legitimate speed-up is the quantum one.

The intended pipeline:

```
  curves (sage/oath{N}_params.json)
             |
             v
  +------------------------------------+
  | oathbreaker/crates/group-action-   |   reversible EC arithmetic:
  | circuit                             |   Karatsuba mul, Binary GCD inv,
  |                                     |   Jacobian point add, windowed
  |                                     |   scalar mul. Measured at
  |                                     |   Oath-8/16/32, projected for
  |                                     |   Oath-64+.
  +------------------------------------+
             |
             v  cargo run --release -p benchmark -- export-qasm
             v  (OpenQASM 3.0 artefact)
             v
  +------------------------------------+
  | oathN_hardware_runner.py           |   Qiskit side: load the QASM,
  | (this directory)                    |   transpile against a real IBM
  |                                     |   backend, submit via
  |                                     |   qiskit_ibm_runtime.SamplerV2,
  |                                     |   classical Shor-spectrum
  |                                     |   post-processing.
  +------------------------------------+
```

The Rust crate already emits OpenQASM 3.0 for Oath-8/16/32 (see the
`export-qasm` target in the benchmark binary). What it does not do yet is
the hardware-side execution -- that stub lives here as
[`oathN_hardware_runner.py`](oathN_hardware_runner.py), awaiting the
hardware era in which these circuits fit.

### Why this is currently a stub

The real-path circuits are far above today's NISQ fidelity budget:

| Tier | Logical qubits | Toffoli | Era |
| --- | ---: | ---: | --- |
| Oath-8 | 210 | 112 K | future FT |
| Oath-16 | 402 | 929 K | future FT |
| Oath-32 | 1 026 | 5.76 M | future FT |
| Oath-64 | ~2 052 (proj.) | ~35 M (proj.) | future FT |

These are not runnable on Eagle or Heron in any meaningful sense. The
runner is therefore written defensively: it accepts a QASM artefact,
reports the resource footprint, and refuses to submit unless the target
backend advertises enough qubits and coherence. Once fault-tolerant
devices become available, the same runner stays.

### What is **not** allowed in this directory

- Any function that reduces to "first solve the discrete log classically,
  then bake it into the circuit."
- Any synthesis path that assumes the group is small enough to enumerate.
- Any "optimization" whose correctness depends on the specific secret `k`.

The POC subdirectory is allowed to do all of those things. This directory
is not.

## Where to start

- Hack on the NISQ software stack, demonstrations, or scaling
  experiments → [`poc/`](poc/).
- Work on the real attack pipeline (QASM emission, hardware runner,
  error-corrected scheduling) → this directory.
- For the architectural rationale and the constraint that separates the
  two paths, see
  [`../docs/NISQ_ROADMAP.md`](../docs/NISQ_ROADMAP.md).
