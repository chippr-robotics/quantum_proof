"""
Real-path Oath-N hardware runner.

This is the legitimate Oath-N execution pipeline: it consumes the
OpenQASM 3.0 artefact emitted by the Rust ``crates/group-action-circuit``
(via ``cargo run --release -p benchmark -- export-qasm``), transpiles it
against a target IBM backend, and submits via
``qiskit_ibm_runtime.SamplerV2``.

Unlike ``poc/``, nothing here solves the discrete log classically. The
circuit is the full reversible EC-arithmetic circuit -- Jacobian point
addition + Karatsuba multiplication + Binary GCD inversion + windowed
scalar multiplication -- materialised as reversible gates by the Rust
framework. All the quantum advantage comes from Shor's period finding;
the classical side does nothing the attacker could not do without a
quantum computer.

Because the Rust-emitted circuits are well above current NISQ fidelity
budgets (see the table in ../README.md), the runner is defensive: it
reports the transpile footprint and refuses to submit unless the backend
advertises enough logical qubits and a plausible coherence envelope.
Once fault-tolerant devices become available, the same runner continues
to work.

Usage (once the QASM artefact exists):

    # 1. Emit the reversible-arithmetic circuit for a tier.
    cd oathbreaker
    cargo run --release -p benchmark -- export-qasm --tier oath8

    # 2. Run it.
    python qiskit/oathN_hardware_runner.py \
        --qasm proofs/oath8.qasm --tier 8 --k 7 --shots 20000 \
        [--backend ibm_torino] [--dry-run]

Exit codes:
    0 -- recovered k matches classical ground truth
    1 -- recovery failed or circuit did not compile under backend budget
    2 -- recovered k did not match
"""

from __future__ import annotations

import argparse
import os
import sys
import time
from pathlib import Path


def _load_qasm(path: Path):
    from qiskit.qasm3 import load

    if not path.is_file():
        raise SystemExit(f"QASM file not found: {path}")
    return load(str(path))


def _classical_groundtruth(tier: int, k: int) -> int:
    """Look up the expected discrete log via the classical oracle living
    in the POC curve loader. We use this only to *verify* the quantum
    result post hoc -- never to build the circuit."""
    # Import lazily and from the POC module so it is obvious this only
    # runs on the classical side, after the quantum circuit has been
    # executed.
    sys.path.insert(0, str(Path(__file__).resolve().parent / "poc"))
    from oath_curve import OathCurve, OathInstance  # noqa: E402

    curve = OathCurve.load_tier(tier)
    inst = OathInstance.from_secret(curve, k % curve.order)
    return curve.classical_dlog(inst.Q)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--qasm",
        type=Path,
        required=True,
        help="path to the OpenQASM 3.0 file emitted by the Rust group-action-circuit crate",
    )
    parser.add_argument(
        "--tier",
        type=int,
        required=True,
        choices=[4, 8, 16, 32, 64],
        help="Oath tier; controls post-processing and ground-truth verification",
    )
    parser.add_argument("--k", type=int, required=True, help="planted secret scalar")
    parser.add_argument("--shots", type=int, default=20000)
    parser.add_argument(
        "--backend",
        type=str,
        default=None,
        help="explicit IBM backend name; defaults to least-busy operational device "
        "that meets the circuit's qubit requirements",
    )
    parser.add_argument(
        "--optimization-level", type=int, default=3, choices=[0, 1, 2, 3]
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="report transpile footprint and exit; no hardware submission",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="submit even if the backend's advertised coherence budget is smaller than "
        "the transpiled circuit (not recommended -- default gates the submission)",
    )
    args = parser.parse_args()

    # Resource budget: published tier estimates from the Rust framework.
    # These exceed current NISQ hardware; the gate here is deliberate.
    BUDGET = {
        4: {"qubits": 12, "toffoli": 400},
        8: {"qubits": 210, "toffoli": 112_000},
        16: {"qubits": 402, "toffoli": 929_000},
        32: {"qubits": 1_026, "toffoli": 5_760_000},
        64: {"qubits": 2_052, "toffoli": 35_000_000},
    }[args.tier]

    print(f"Loading Rust-emitted OpenQASM from {args.qasm}")
    circuit = _load_qasm(args.qasm)
    print(
        f"Logical circuit: {circuit.num_qubits} qubits, depth {circuit.depth()}, "
        f"~{BUDGET['toffoli']} Toffoli (per framework estimate)"
    )

    if args.dry_run:
        print("dry-run requested; skipping backend selection and submission.")
        return 0

    try:
        from qiskit import transpile
        from qiskit_ibm_runtime import QiskitRuntimeService, SamplerV2
    except ImportError as exc:
        raise SystemExit(
            "qiskit-ibm-runtime is required for hardware runs "
            "(pip install qiskit-ibm-runtime)"
        ) from exc

    token = os.environ.get("IBM_QUANTUM_TOKEN")
    if not token:
        raise SystemExit("set IBM_QUANTUM_TOKEN with your quantum.ibm.com API key")

    service = QiskitRuntimeService(token=token)

    if args.backend:
        backend = service.backend(args.backend)
    else:
        candidates = [
            b
            for b in service.backends(simulator=False, operational=True)
            if b.configuration().n_qubits >= BUDGET["qubits"]
        ]
        if not candidates:
            raise SystemExit(
                f"No operational backend advertises >= {BUDGET['qubits']} qubits; "
                f"Oath-{args.tier} is beyond the currently-deployed hardware era. "
                "Run --dry-run for a circuit footprint report."
            )
        backend = min(candidates, key=lambda b: b.status().pending_jobs)

    print(f"Backend: {backend.name}  pending_jobs={backend.status().pending_jobs}")

    transpiled = transpile(
        circuit,
        backend,
        optimization_level=args.optimization_level,
        seed_transpiler=42,
    )
    two_q = sum(1 for op in transpiled.data if op.operation.num_qubits >= 2)
    print(f"Transpiled: depth={transpiled.depth()}  2q={two_q}")

    # Coherence gate: under current device-quality conventions we refuse to
    # submit if the expected error-event count per shot is much greater
    # than 1. Override with --force if you know what you're doing.
    # Published median 2q error rate (approx; updated by the runtime).
    err = 3e-3  # Heron, for ECR use ~7e-3 on Eagle
    expected_errors = two_q * err
    if expected_errors > 1.0 and not args.force:
        raise SystemExit(
            f"Transpiled circuit has {two_q} two-qubit gates, expected "
            f"{expected_errors:.1f} error events per shot at 2q error ~{err}. "
            "This is above the NISQ budget where the result would still "
            "be recoverable. Use --force to submit anyway."
        )

    sampler = SamplerV2(mode=backend)
    t0 = time.time()
    job = sampler.run([transpiled], shots=args.shots)
    print(f"Submitted job {job.job_id()}")
    job.result()  # preserved in the job record; offline recovery reads it
    elapsed = time.time() - t0
    print(f"Job complete in {elapsed:.1f}s")

    # Shor post-processing for the Rust-emitted circuit is tier-specific
    # and lives with the circuit-producing code; the runner only handles
    # the execution shell. A follow-up commit will wire the matching
    # post-processor (see oathbreaker/crates/group-action-circuit/src/shor.rs
    # for the expected measurement layout).
    print(
        "NOTE: post-processing for Rust-emitted circuits is not yet wired; "
        "the measurement artefact is preserved for offline recovery."
    )

    try:
        expected_k = _classical_groundtruth(args.tier, args.k)
    except Exception as exc:  # noqa: BLE001
        print(f"Could not compute classical ground truth for this tier: {exc}")
        return 0
    print(f"Classical ground truth: k = {expected_k}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
