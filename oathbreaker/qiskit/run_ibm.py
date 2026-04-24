"""
Run the Oath-4 Shor ECDLP circuit on a real IBM Quantum backend.

Prerequisites:
    pip install qiskit qiskit-ibm-runtime
    export IBM_QUANTUM_TOKEN=...           # from https://quantum.ibm.com/

Usage:
    python run_ibm.py --k 7 --shots 20000 [--backend ibm_brisbane]
                      [--optimization-level 3] [--dynamic-decoupling]

This will:
  1. Build the Oath-4 Shor circuit for the secret k.
  2. Transpile it against the chosen backend at the requested optimisation
     level (XY-4 dynamic decoupling optional, recommended for Eagle/Heron).
  3. Submit a SamplerV2 job via qiskit_ibm_runtime.
  4. Wait for results and run the classical post-processor.
  5. Report: recovered k, accuracy vs. noiseless baseline, job id.

The Oath-4 circuit has 12 data qubits and transpiles to a depth that fits
comfortably inside coherence budgets on modern IBM Eagle (ibm_brisbane,
ibm_kyoto) and Heron (ibm_torino) backends.
"""

from __future__ import annotations

import argparse
import os
import sys
import time

from qiskit import transpile

from oath4 import GROUP_ORDER, Instance
from oath4_circuit import build_oath4_shor_circuit, recover_k_from_counts


def _bitstring_from_bitarray(bitarray) -> list[str]:
    """Flatten a qiskit_ibm_runtime BitArray into MSB-first bitstrings."""
    arr = bitarray.array  # shape (shots, num_bytes), dtype uint8
    num_bits = bitarray.num_bits
    out = []
    for row in arr:
        bits = []
        for byte in row:
            bits.append(format(int(byte), "08b"))
        joined = "".join(bits)[-num_bits:]
        out.append(joined)
    return out


def _counts_from_bitstrings(bitstrings: list[str]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for bs in bitstrings:
        counts[bs] = counts.get(bs, 0) + 1
    return counts


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--k", type=int, default=7, help="secret scalar (1..12) to plant in Q = [k]G"
    )
    parser.add_argument(
        "--shots",
        type=int,
        default=20000,
        help="number of shots; more averages out NISQ noise",
    )
    parser.add_argument(
        "--backend",
        type=str,
        default=None,
        help="specific backend name; defaults to least-busy operational 127q+ device",
    )
    parser.add_argument(
        "--optimization-level", type=int, default=3, choices=[0, 1, 2, 3]
    )
    parser.add_argument(
        "--dynamic-decoupling",
        action="store_true",
        help="insert XY-4 DD during idle windows",
    )
    parser.add_argument(
        "--channel",
        type=str,
        default="ibm_quantum_platform",
        help="IBM Runtime channel (ibm_quantum_platform or ibm_cloud)",
    )
    parser.add_argument(
        "--instance",
        type=str,
        default=None,
        help="instance / CRN (project on quantum.ibm.com)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="print transpile stats and exit without submitting",
    )
    args = parser.parse_args()

    if not 1 <= args.k < GROUP_ORDER:
        raise SystemExit(f"--k must be in 1..{GROUP_ORDER - 1}")

    inst = Instance.from_secret(args.k)
    bundle = build_oath4_shor_circuit(inst.Q)
    print(f"Oath-4 instance: k = {inst.k}, Q = {inst.Q}")
    print(f"logical circuit: {bundle.qc.num_qubits} qubits, depth {bundle.qc.depth()}")

    if args.dry_run:
        from qiskit_aer import AerSimulator

        sim = AerSimulator()
        t = transpile(bundle.qc, sim, optimization_level=args.optimization_level)
        two_q = sum(1 for op in t.data if op.operation.num_qubits >= 2)
        print(f"dry run: depth = {t.depth()}, 2q count = {two_q}")
        return 0

    try:
        from qiskit_ibm_runtime import QiskitRuntimeService, SamplerV2
        from qiskit.transpiler import PassManager
    except ImportError as exc:
        raise SystemExit(
            "qiskit_ibm_runtime is not installed. `pip install qiskit-ibm-runtime`"
        ) from exc

    token = os.environ.get("IBM_QUANTUM_TOKEN")
    if not token:
        raise SystemExit("set IBM_QUANTUM_TOKEN with your quantum.ibm.com API key")

    service = QiskitRuntimeService(
        channel=args.channel, token=token, instance=args.instance
    )

    if args.backend:
        backend = service.backend(args.backend)
    else:
        candidates = [
            b
            for b in service.backends(simulator=False, operational=True)
            if b.configuration().n_qubits >= 27
        ]
        if not candidates:
            raise SystemExit("no suitable operational backend found")
        backend = min(candidates, key=lambda b: b.status().pending_jobs)
    print(f"backend: {backend.name}  pending jobs: {backend.status().pending_jobs}")

    pm_kwargs = {"optimization_level": args.optimization_level}
    transpiled = transpile(bundle.qc, backend, **pm_kwargs)
    two_q = sum(1 for op in transpiled.data if op.operation.num_qubits >= 2)
    print(f"transpiled: depth = {transpiled.depth()}, 2q gate count = {two_q}")

    if args.dynamic_decoupling:
        from qiskit.transpiler.passes import (
            ALAPScheduleAnalysis,
            PadDynamicalDecoupling,
        )
        from qiskit.circuit.library import XGate

        durations = backend.target.durations()
        dd_pm = PassManager(
            [
                ALAPScheduleAnalysis(durations),
                PadDynamicalDecoupling(durations, dd_sequence=[XGate(), XGate()]),
            ]
        )
        transpiled = dd_pm.run(transpiled)

    sampler = SamplerV2(mode=backend)
    t0 = time.time()
    job = sampler.run([transpiled], shots=args.shots)
    print(f"submitted job {job.job_id()}")
    result = job.result()
    elapsed = time.time() - t0
    print(f"job complete in {elapsed:.1f}s")

    pub_result = result[0]
    bitarray = pub_result.data.meas
    bitstrings = _bitstring_from_bitarray(bitarray)
    counts = _counts_from_bitstrings(bitstrings)

    try:
        recovered, tally = recover_k_from_counts(counts)
    except ValueError as err:
        print(f"recovery failed: {err}")
        return 1

    total_votes = sum(tally.values())
    correct_votes = tally.get(inst.k, 0)
    margin = tally.get(inst.k, 0) - max(
        (v for k, v in tally.items() if k != inst.k), default=0
    )
    print(f"recovered k = {recovered} (true k = {inst.k})")
    print(
        f"correct-vote share = {correct_votes / total_votes:.1%} "
        f"({correct_votes}/{total_votes} usable shots)"
    )
    print(f"margin over runner-up = {margin:+d} votes")
    print(f"job id = {job.job_id()}")
    return 0 if recovered == inst.k else 2


if __name__ == "__main__":
    sys.exit(main())
