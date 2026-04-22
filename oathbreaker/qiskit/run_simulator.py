"""
Noiseless AerSimulator run of the Oath-4 Shor ECDLP circuit.

Usage: python run_simulator.py [--k 7] [--shots 4096]

This is the pre-flight check that should pass with near 100% recovery before
committing hardware time. If this fails, the hardware run cannot succeed.
"""

from __future__ import annotations

import argparse

from qiskit import transpile
from qiskit_aer import AerSimulator

from oath4 import GROUP_ORDER, Instance
from oath4_circuit import build_oath4_shor_circuit, recover_k_from_counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--k", type=int, default=7,
                        help="secret scalar to plant in Q = [k]G (1..12)")
    parser.add_argument("--shots", type=int, default=4096)
    parser.add_argument("--seed", type=int, default=12345)
    args = parser.parse_args()

    if not 1 <= args.k < GROUP_ORDER:
        raise SystemExit(f"--k must be in 1..{GROUP_ORDER - 1}")

    inst = Instance.from_secret(args.k)
    bundle = build_oath4_shor_circuit(inst.Q)
    print(f"Oath-4 instance: k = {inst.k}, Q = {inst.Q}")
    print(f"circuit: {bundle.qc.num_qubits} qubits, depth {bundle.qc.depth()}")

    sim = AerSimulator(seed_simulator=args.seed)
    tqc = transpile(bundle.qc, sim, optimization_level=2)
    print(f"transpiled depth (statevector basis): {tqc.depth()}")

    result = sim.run(tqc, shots=args.shots).result()
    counts = result.get_counts()
    total = sum(counts.values())
    print(f"shots = {total}, distinct outcomes = {len(counts)}")

    recovered, tally = recover_k_from_counts(counts)
    top = sorted(tally.items(), key=lambda kv: -kv[1])[:5]
    print("top k candidates: " + ", ".join(f"k={k} ({v})" for k, v in top))
    print(f"recovered k = {recovered}  (true k = {inst.k})")

    if recovered == inst.k:
        print("PASS: Oath-4 Shor ECDLP recovered the secret.")
        return 0
    print("FAIL: recovered k does not match ground truth.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
