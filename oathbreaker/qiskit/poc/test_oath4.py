"""
End-to-end noiseless test suite for the Oath-4 Shor circuit.

Exercises every non-zero secret k in 1..n-1 against both the baseline and the
hardware-optimized (Beauregard QFT adder) builders. Run with:

    python test_oath4.py
"""

from __future__ import annotations

import sys
from typing import Callable

from qiskit import transpile
from qiskit_aer import AerSimulator

from oath4 import GROUP_ORDER, Instance, classical_dlog
from oath4_circuit import build_oath4_shor_circuit, recover_k_from_counts
from oath4_optimized import build_oath4_shor_circuit_optimized


def _sweep(
    label: str,
    builder: Callable,
    sim: AerSimulator,
    shots: int,
) -> list[tuple[int, int, float]]:
    """Run the Shor recovery on every k in 1..n-1 and return failures."""
    print(f"\n--- {label} ---")
    failures: list[tuple[int, int, float]] = []
    for k in range(1, GROUP_ORDER):
        inst = Instance.from_secret(k)
        classical_k = classical_dlog(inst.Q)
        assert classical_k == k, f"classical oracle disagrees for k={k}"

        bundle = builder(inst.Q)
        tqc = transpile(bundle.qc, sim, optimization_level=1)
        counts = sim.run(tqc, shots=shots).result().get_counts()
        recovered, tally = recover_k_from_counts(counts)
        top_share = tally[recovered] / sum(tally.values())
        status = "PASS" if recovered == k else "FAIL"
        if recovered != k:
            failures.append((k, recovered, top_share))
        print(
            f"  k={k:2d}  Q={str(inst.Q):<9s}  recovered={recovered:2d}  "
            f"peak={top_share:.1%}  {status}"
        )
    return failures


def test_all_ks(shots: int = 2048) -> int:
    sim = AerSimulator(seed_simulator=202604)

    baseline_fails = _sweep(
        "baseline (UnitaryGate.control)",
        build_oath4_shor_circuit,
        sim,
        shots,
    )
    optimized_fails = _sweep(
        "optimized (Beauregard QFT adder)",
        build_oath4_shor_circuit_optimized,
        sim,
        shots,
    )

    print()
    if baseline_fails:
        print(f"baseline FAILED on {len(baseline_fails)} values: {baseline_fails}")
    if optimized_fails:
        print(f"optimized FAILED on {len(optimized_fails)} values: {optimized_fails}")
    if baseline_fails or optimized_fails:
        return 1
    print(f"All {GROUP_ORDER - 1} Oath-4 secrets recovered via both builders.")
    return 0


if __name__ == "__main__":
    sys.exit(test_all_ks())
