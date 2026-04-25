"""
Backend metadata catalogue for the Oathbreaker quantum execution stack.

Each ``BackendSpec`` carries the information that compilers, runners, and
fidelity gates need to pick a target without hard-coding magic numbers:

  - the underlying Qiskit-style backend object (real or fake)
  - the native two-qubit gate set, used by TKET to choose its rebase
    target without round-tripping through Qiskit's transpile
  - the published two-qubit gate error and the typical T2 coherence
    time, used by ``oathN_hardware_runner.py`` to gate submission
  - a flag for all-to-all connectivity (matters for Quantinuum
    H-series in a follow-up commit; unused at IBM tiers)

This module is intentionally light. Bigger metadata layers (per-qubit
calibration, dynamic-decoupling strategies, etc.) are the runner's job;
``BackendSpec`` is just the catalogue.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Callable, Optional


@dataclass(frozen=True)
class BackendSpec:
    """A target backend for the Oathbreaker compile / submit pipeline."""

    name: str
    family: str  # "ibm-eagle" | "ibm-heron" | "quantinuum-h" | ...
    factory: Callable  # () -> backend object (Qiskit-style)
    two_qubit_gates: tuple[str, ...]  # native 2q gate names
    one_qubit_gates: tuple[str, ...]  # native 1q gate names (for TKET rebase)
    two_qubit_error: float  # published median 2q gate error
    t2_microseconds: float  # typical T2 (us)
    all_to_all: bool  # True iff the device has all-to-all connectivity
    is_simulator: bool  # True for fake / emulator backends (no submission)

    def make_backend(self):
        """Instantiate the backend object lazily."""
        return self.factory()


def _fake_brisbane():
    from qiskit_ibm_runtime.fake_provider import FakeBrisbane

    return FakeBrisbane()


def _fake_torino():
    from qiskit_ibm_runtime.fake_provider import FakeTorino

    return FakeTorino()


# Catalogue. Numbers are published median values as of April 2026; the runner
# will refresh from `backend.target` properties when a live backend is used.
_REGISTRY: dict[str, BackendSpec] = {
    "fake_brisbane": BackendSpec(
        name="fake_brisbane",
        family="ibm-eagle",
        factory=_fake_brisbane,
        two_qubit_gates=("ecr",),
        one_qubit_gates=("rz", "sx", "x"),
        two_qubit_error=7.0e-3,
        t2_microseconds=200.0,
        all_to_all=False,
        is_simulator=True,
    ),
    "fake_torino": BackendSpec(
        name="fake_torino",
        family="ibm-heron",
        factory=_fake_torino,
        two_qubit_gates=("cz",),
        one_qubit_gates=("rz", "sx", "x"),
        two_qubit_error=3.0e-3,
        t2_microseconds=300.0,
        all_to_all=False,
        is_simulator=True,
    ),
}


def get_backend_spec(name: str) -> BackendSpec:
    if name not in _REGISTRY:
        raise ValueError(f"unknown backend '{name}'; available: {sorted(_REGISTRY)}")
    return _REGISTRY[name]


def available_backends() -> list[str]:
    return sorted(_REGISTRY)


def register_backend(spec: BackendSpec) -> None:
    """Public hook so future commits (Quantinuum, live IBM) can extend the
    catalogue without modifying this module."""
    _REGISTRY[spec.name] = spec


def shortname_to_spec(short: str) -> Optional[BackendSpec]:
    """Map the convenience CLI shortnames used by the POC scripts to specs."""
    aliases = {
        "brisbane": "fake_brisbane",
        "torino": "fake_torino",
    }
    full = aliases.get(short, short)
    return _REGISTRY.get(full)
