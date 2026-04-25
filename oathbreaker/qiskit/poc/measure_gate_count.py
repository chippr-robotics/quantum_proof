"""
Measure compiled 2q gate count and depth for the Oath-N Shor ECDLP circuit
across tiers, modular-adder methods, and compiler strategies.

Usage:
    python measure_gate_count.py [--tiers 4,8,16] [--k 7] [--opt-level 3]
                                 [--compilers qiskit,tket]
                                 [--backends torino,brisbane]

Prints a table of {Oath-N} x {qft_beauregard, cdkm_ripple} x
{compilers} x {backends}. No credentials or network needed; uses
Qiskit's published fake-backend models.
"""

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path

# Allow running from poc/ without installing the parent package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from backends import (  # noqa: E402
    available_backends,
    get_backend_spec,
    shortname_to_spec,
)
from compilers import available_compilers, get_compiler  # noqa: E402
from modular_adders import available_methods  # noqa: E402
from oath_curve import OathCurve, OathInstance  # noqa: E402
from oathN_circuit import build_oathN_shor_circuit  # noqa: E402


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
        default="torino",
        help=(
            "comma-separated backend shortnames (torino, brisbane) or "
            f"full names from {available_backends()}"
        ),
    )
    parser.add_argument(
        "--compilers",
        type=str,
        default=",".join(available_compilers()),
        help=f"comma-separated compiler names from {available_compilers()}",
    )
    args = parser.parse_args()

    tiers = [int(t) for t in args.tiers.split(",") if t.strip()]
    compilers = [get_compiler(c) for c in args.compilers.split(",") if c.strip()]
    specs = []
    for shortname in args.backends.split(","):
        shortname = shortname.strip()
        if not shortname:
            continue
        spec = shortname_to_spec(shortname) or get_backend_spec(shortname)
        specs.append(spec)

    methods = available_methods()

    header = (
        f"{'backend':<16s} {'compiler':<10s} {'tier':<7s} {'method':<16s} "
        f"{'qubits':>6s} {'2q':>8s} {'depth':>8s} {'t':>7s}"
    )
    print(header)
    print("-" * len(header))

    for spec in specs:
        for tier in tiers:
            curve = OathCurve.load_tier(tier)
            k = args.k % curve.order or 1
            inst = OathInstance.from_secret(curve, k)
            for method in methods:
                bundle = build_oathN_shor_circuit(curve, inst.Q, adder_method=method)
                for compiler in compilers:
                    t0 = time.time()
                    cc = compiler.compile(
                        bundle.qc,
                        spec,
                        optimization_level=args.opt_level,
                        seed=args.seed,
                    )
                    elapsed = time.time() - t0
                    print(
                        f"{spec.name:<16s} {compiler.name:<10s} "
                        f"Oath-{tier:<3d} {method:<16s} "
                        f"{bundle.qc.num_qubits:>6d} {cc.two_qubit_count:>8d} "
                        f"{cc.depth:>8d} {elapsed:>6.1f}s"
                    )
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
