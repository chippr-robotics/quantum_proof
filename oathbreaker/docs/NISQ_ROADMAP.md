# Oathbreaker NISQ Roadmap -- Constraints and Non-Constraints

This document exists because it is *easy* to write a quantum-looking
circuit that recovers an ECDLP secret only because the secret was
classically solved at circuit-construction time. The resulting
"demonstration" offers no quantum advantage and teaches nothing about
what will actually be required to attack a cryptographically-sized curve.

This is a specification for what the `oathbreaker/qiskit/` tree must and
must not do as it scales from the Oath-4 proof of concept up to Oath-N.

## Hard constraints on the real-Oath-N path

The codebase is split into two zones:

- `oathbreaker/qiskit/poc/` -- proof-of-concept NISQ demonstrations.
  These are *allowed* to precompute, enumerate, or otherwise bake
  classical information about the secret into the circuit. They exist
  to validate the Qiskit / IBM software stack at tiny sizes.

- `oathbreaker/qiskit/` -- the real Oath-N execution path. This is
  *not* allowed to use any of the following:

  1. **Classical discrete-log oracles during circuit construction.** The
     circuit for `Q = [k]G` must be constructible from `(curve, G, Q)`
     alone. Any step that returns an integer `i` such that `[i]G = Q`
     (or `[i]G = [c]Q` for any `c`) is forbidden outside of ground-truth
     verification *after* the hardware result has been returned.
  2. **Synthesis passes whose cost scales with `order` or `p`.** Linear
     scans over the group are fine in a POC that stops at 65K points;
     they are not fine in the real path, which must target orders
     beyond what a classical machine can enumerate.
  3. **Circuit-builder shortcuts that depend on the specific secret
     value `k` being compiled.** A single circuit for bit width `n`
     should work for any `Q` of that width; it must not be regenerated
     per `k`. (This is what rules out "I compiled the right answer into
     the gates" demonstrations.)
  4. **Encodings that rely on knowing the group-element --> integer
     isomorphism.** In particular: storing EC points as their
     `[i]G` index in quantum registers requires precomputing `i`, which
     is the forbidden ECDLP lookup. Real circuits carry EC points as
     their `(x, y)` coordinates (or their Jacobian `(X, Y, Z)`
     projective form) and add them via reversible field arithmetic.

## What the real path must use

The only legitimate quantum advantage for Oath-N is **Shor period
finding over genuine reversible EC arithmetic**. Concretely:

- Reversible field arithmetic over `GF(p)`: adders, multipliers, and
  modular inversion expressed as sequences of NOT / CNOT / Toffoli
  gates. Oathbreaker's implementations live in
  `oathbreaker/crates/reversible-arithmetic/`.
- Reversible elliptic-curve point operations built on top of that
  arithmetic: Jacobian point addition and doubling, windowed scalar
  multiplication with one-hot QROM decode. These live in
  `oathbreaker/crates/group-action-circuit/`.
- Coherent double-scalar multiplication `[a]G + [b]Q` with `a, b` in
  superposition, followed by independent inverse QFTs on `a` and `b`.
  Measuring the exponent registers produces Shor's spectrum exactly as
  at Oath-4, but without any classical knowledge of `k`.

The Rust group-action-circuit crate already materialises this stack for
Oath-8/16/32 (measured) and Oath-64 (projected). It emits OpenQASM 3.0
via `cargo run --release -p benchmark -- export-qasm`, and that QASM is
what the `qiskit/oathN_hardware_runner.py` pipeline is designed to
execute.

## NISQ-feasibility tracking

Real-path Oath-N is **not** NISQ-feasible today. The resource counts
from the Rust framework make that explicit:

| Tier | Logical qubits | Toffoli | NISQ-feasible? |
| --- | ---: | ---: | --- |
| Oath-4 (POC) | 12 | ~400 | yes (validated) |
| Oath-8 | 210 | 112 K | no |
| Oath-16 | 402 | 929 K | no |
| Oath-32 | 1 026 | 5.76 M | no |
| Oath-64 | ~2 052 | ~35 M | no |

What NISQ-era progress looks like is *not* "run Oath-8 on Heron today"
-- those gate counts are several orders of magnitude over the fidelity
budget. It looks like:

1. Shrinking the Oath-N Toffoli count further (the Oathbreaker v3
   optimizations continue that). Every one of these improvements must
   still satisfy the constraints above.
2. Waiting for error-corrected hardware where the per-logical-operation
   error rate is low enough that `10^6` Toffolis is a reasonable
   budget. This is the same fault-tolerant threshold everyone is
   waiting for.
3. New algorithmic work at the protocol level (Eker-Hastad small-`d`
   variants, semiclassical IQPE, approximate QFT) that reduces the
   Shor-protocol overhead without compromising the reversible EC
   arithmetic underneath. The POC directory is a good place to
   prototype the protocol side; the real path adopts a protocol change
   only if it preserves the hard constraints above.

## Reviewing new code against the constraints

A PR touching `oathbreaker/qiskit/` (outside `poc/`) is reviewable
against the following checklist:

- [ ] Does this code read the secret `k` at any point?
- [ ] Does this code call anything equivalent to `point_to_index`,
      `classical_dlog`, `discrete_log`, `pollard_rho`, `bsgs`, or a
      Sage/SymPy group-element enumeration?
- [ ] Does the circuit vary with `k` beyond the `Q` input?
- [ ] Does the synthesis cost of this code scale with `order` or `p`
      (as opposed to `log(order)`)?

If the answer to any of the above is yes, the code belongs in `poc/`
(or is a bug).

## The value of the POC

The POC path is not a throwaway. It is how we validate:

- Qiskit transpile quality across Eagle and Heron basis sets
- SamplerV2 hardware submission
- Dynamic circuits / mid-circuit measurement / classical feedforward
- Error mitigation layering (DD, ZNE, PEC, twirling)
- The Shor spectrum post-processor and classical recovery pipeline

These components are all reusable in the real path. Keeping the POC
cleanly labelled means we can iterate on them without blurring the line
between "our software ran" and "we broke a curve."
