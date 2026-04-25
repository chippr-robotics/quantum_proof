"""
Correctness gate for the Oathbreaker Compiler strategies.

For every supported (compiler, adder) pair we transpile the Oath-4 Shor
circuit against ``fake_torino`` and run it on AerSimulator, sweeping a
sampler set of secrets ``{1, 7, 12}`` (lower boundary, middle, upper
boundary). The Compiler interface is a contract that says "preserve
circuit semantics"; this test enforces it.

This is a CI-blocking test -- a TKET regression that breaks the Shor
recovery here is exactly what we want to catch before any depth /
gate-count claim.

Pass ``--full`` to sweep every k in 1..12 instead of the sampler set.
The full sweep takes roughly seven minutes on the CI runner because
the TKET pipeline is ~16 s per Oath-4 compile.

Run with:  python test_compilers.py [--full]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Allow direct execution from poc/ without installing the parent package.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from qiskit_aer import AerSimulator  # noqa: E402

from backends import get_backend_spec  # noqa: E402
from compilers import available_compilers, get_compiler  # noqa: E402
from modular_adders import available_methods  # noqa: E402
from oath4 import GROUP_ORDER, Instance  # noqa: E402
from oath4_circuit import recover_k_from_counts  # noqa: E402
from oath4_optimized import build_oath4_shor_circuit_optimized  # noqa: E402


SAMPLER_KS: tuple[int, ...] = (1, 7, GROUP_ORDER - 1)


def main(shots: int = 2048) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--full",
        action="store_true",
        help="sweep every k in 1..12 instead of the {1,7,12} sampler",
    )
    args = parser.parse_args()
    ks = tuple(range(1, GROUP_ORDER)) if args.full else SAMPLER_KS

    sim = AerSimulator(seed_simulator=202604)
    spec = get_backend_spec("fake_torino")

    failures: list[tuple[str, str, int, int]] = []
    for compiler_name in available_compilers():
        compiler = get_compiler(compiler_name)
        for adder_method in available_methods():
            print(f"\n--- compiler={compiler_name}  adder={adder_method} ---")
            for k in ks:
                inst = Instance.from_secret(k)
                bundle = build_oath4_shor_circuit_optimized(
                    inst.Q, adder_method=adder_method
                )
                cc = compiler.compile(bundle.qc, spec, optimization_level=3, seed=42)
                counts = sim.run(cc.circuit, shots=shots).result().get_counts()
                rec, tally = recover_k_from_counts(counts)
                top = tally[rec] / sum(tally.values())
                ok = rec == k
                status = "PASS" if ok else "FAIL"
                if not ok:
                    failures.append((compiler_name, adder_method, k, rec))
                print(
                    f"  k={k:2d}  recovered={rec:2d}  peak={top:.1%}  "
                    f"2q={cc.two_qubit_count}  depth={cc.depth}  {status}"
                )

    print()
    if failures:
        print(f"FAILED on {len(failures)} (compiler, adder, k) combinations:")
        for entry in failures:
            print(f"  {entry}")
        return 1
    print(
        "All compiler x adder x k combinations recovered the secret "
        "via the Oathbreaker Shor pipeline."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
