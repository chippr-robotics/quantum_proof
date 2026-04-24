"""
Measure compiled 2q gate count and depth for the Oath-N Shor ECDLP circuit
across tiers and modular-adder methods, on IBM-class fake backends.

Usage:
    python measure_gate_count.py [--tiers 4,8,16] [--k 7] [--opt-level 3]

Prints a table of {Oath-N} x {qft_beauregard, cdkm_ripple} x
{FakeBrisbane (Eagle), FakeTorino (Heron)}. No credentials or network
needed; uses Qiskit's published fake-backend models.
"""

from __future__ import annotations

import argparse
import time

from qiskit import transpile

from modular_adders import available_methods
from oath_curve import OathCurve, OathInstance
from oathN_circuit import build_oathN_shor_circuit


def count_two_q(circ) -> int:
    return sum(1 for op in circ.data if op.operation.num_qubits >= 2)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--tiers",
        type=str,
        default="4",
        help="comma-separated Oath tiers to measure (e.g. 4,8,16)",
    )
    parser.add_argument(
        "--k",
        type=int,
        default=7,
        help="secret scalar used to instantiate Q = [k]G",
    )
    parser.add_argument("--opt-level", type=int, default=3, choices=[0, 1, 2, 3])
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--backends",
        type=str,
        default="brisbane,torino",
        help="comma-separated backend shortnames: brisbane (Eagle), torino (Heron)",
    )
    args = parser.parse_args()

    from qiskit_ibm_runtime.fake_provider import FakeBrisbane, FakeTorino

    backend_map = {"brisbane": FakeBrisbane, "torino": FakeTorino}
    backends = [
        (name, backend_map[name]()) for name in args.backends.split(",") if name.strip()
    ]
    tiers = [int(t) for t in args.tiers.split(",") if t.strip()]
    methods = available_methods()

    print(
        f"{'backend':<28s} {'tier':<7s} {'method':<16s} "
        f"{'qubits':>6s} {'2q':>8s} {'depth':>8s} {'t':>6s}"
    )
    print("-" * 84)
    for be_name, be in backends:
        be_label = f"{be.name}"
        for tier in tiers:
            curve = OathCurve.load_tier(tier)
            k = args.k % curve.order
            if k == 0:
                k = 1
            inst = OathInstance.from_secret(curve, k)
            for method in methods:
                t0 = time.time()
                bundle = build_oathN_shor_circuit(curve, inst.Q, adder_method=method)
                tqc = transpile(
                    bundle.qc,
                    be,
                    optimization_level=args.opt_level,
                    seed_transpiler=args.seed,
                )
                elapsed = time.time() - t0
                two_q = count_two_q(tqc)
                print(
                    f"{be_label:<28s} Oath-{tier:<3d} {method:<16s} "
                    f"{bundle.qc.num_qubits:>6d} {two_q:>8d} {tqc.depth():>8d} "
                    f"{elapsed:>5.1f}s"
                )
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
