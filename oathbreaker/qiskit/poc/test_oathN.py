"""
Generic Oath-N test sweep.

For Oath-4 we run the full noiseless simulator over every secret k in
1..n-1 for every pluggable adder. For Oath-8 and above we only verify
that the circuit BUILDS cleanly, since AerSimulator statevector on
40+ qubits is impractical. The build-level check catches register-width
mismatches, missing ancillae, and typos in the adder dispatch.

Run with:  python test_oathN.py
"""

from __future__ import annotations

import sys

from qiskit import transpile
from qiskit_aer import AerSimulator

from modular_adders import available_methods
from oath_curve import OathCurve, OathInstance
from oathN_circuit import build_oathN_shor_circuit, recover_k_from_counts


def _exhaustive_sweep(tier: int, method: str, shots: int = 2048) -> int:
    curve = OathCurve.load_tier(tier)
    sim = AerSimulator(seed_simulator=202604)
    failures: list[tuple[int, int]] = []
    for k in range(1, curve.order):
        inst = OathInstance.from_secret(curve, k)
        bundle = build_oathN_shor_circuit(curve, inst.Q, adder_method=method)
        tqc = transpile(bundle.qc, sim, optimization_level=1)
        counts = sim.run(tqc, shots=shots).result().get_counts()
        recovered, _ = recover_k_from_counts(counts, curve)
        if recovered != k:
            failures.append((k, recovered))
    return len(failures)


def _build_only(tier: int, method: str) -> None:
    curve = OathCurve.load_tier(tier)
    inst = OathInstance.from_secret(curve, 7 % curve.order)
    bundle = build_oathN_shor_circuit(curve, inst.Q, adder_method=method)
    if bundle.qc.num_qubits <= 0:
        raise RuntimeError(f"Oath-{tier}/{method}: empty circuit")


def main() -> int:
    print("Oath-4 noiseless Shor ECDLP sweep (every k in 1..12):")
    failures = 0
    for method in available_methods():
        f = _exhaustive_sweep(tier=4, method=method)
        failures += f
        status = "PASS" if f == 0 else f"FAIL ({f})"
        print(f"  Oath-4  / {method:<16s}  {status}")

    print("\nOath-8 / Oath-16 build-only check:")
    for tier in (8, 16):
        for method in available_methods():
            try:
                _build_only(tier, method)
                print(f"  Oath-{tier:<3d} / {method:<16s}  builds")
            except Exception as exc:  # noqa: BLE001
                failures += 1
                print(f"  Oath-{tier:<3d} / {method:<16s}  FAIL  ({exc!r})")

    if failures:
        print(f"\n{failures} failure(s)")
        return 1
    print("\nAll Oath-N configurations OK.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
