"""
Measure compiled 2q gate count and depth for the Oath-4 circuit on IBM-class
backends, comparing the baseline UnitaryGate implementation with the
Beauregard QFT adder optimization.

Usage:
    python measure_gate_count.py [--k 7] [--opt-level 3] [--seed 42]

Prints a table of {baseline, optimized} x {FakeBrisbane (Eagle), FakeTorino
(Heron)} with 2q gate counts and circuit depths. No hardware credentials or
network access are required.
"""

from __future__ import annotations

import argparse

from qiskit import transpile

from oath4 import Instance
from oath4_circuit import build_oath4_shor_circuit
from oath4_optimized import build_oath4_shor_circuit_optimized


def count_two_q(circ) -> int:
    return sum(1 for op in circ.data if op.operation.num_qubits >= 2)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--k",
        type=int,
        default=7,
        help="secret scalar (1..12) used to instantiate Q = [k]G",
    )
    parser.add_argument("--opt-level", type=int, default=3, choices=[0, 1, 2, 3])
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    from qiskit_ibm_runtime.fake_provider import FakeBrisbane, FakeTorino

    inst = Instance.from_secret(args.k)
    builders = [
        ("baseline (UnitaryGate.control)", build_oath4_shor_circuit),
        ("optimized (Beauregard QFT)", build_oath4_shor_circuit_optimized),
    ]
    backends = [
        ("FakeBrisbane (Eagle, ECR)", FakeBrisbane),
        ("FakeTorino (Heron, CZ)", FakeTorino),
    ]

    print(f"Oath-4 Shor ECDLP circuit for Q = [{inst.k}]G\n")
    print(f"{'backend':<30s} {'builder':<35s} {'2q':>6s} {'depth':>7s}")
    print("-" * 80)
    for be_name, be_cls in backends:
        be = be_cls()
        for builder_name, builder in builders:
            bundle = builder(inst.Q)
            tqc = transpile(
                bundle.qc,
                be,
                optimization_level=args.opt_level,
                seed_transpiler=args.seed,
            )
            two_q = count_two_q(tqc)
            print(f"{be_name:<30s} {builder_name:<35s} {two_q:>6d} {tqc.depth():>7d}")
        print()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
