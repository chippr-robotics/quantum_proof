"""
Pluggable compiler strategies for the Oathbreaker Qiskit pipeline.

Both the POC (``poc/``) and the real Oath-N runner go through this module
to compile a logical Qiskit ``QuantumCircuit`` against a target backend.
Two strategies are provided:

  - ``QiskitCompiler``  -- the existing path. ``qiskit.transpile`` with the
    backend, optimisation level and seed.
  - ``TketCompiler``    -- pytket pipeline: import, decompose boxes,
    rebase to the backend's native gate set, run the full peephole
    optimiser, route against the backend's coupling map, rebase again,
    remove redundancies. Then a final Qiskit ``transpile`` at
    optimisation level 0 to harmonise the metadata back into Qiskit form
    for downstream consumers (samplers, transpile-aware diagnostics).

Both strategies return a ``CompiledCircuit`` with the transpiled circuit
and the relevant per-backend statistics. Adders / runners pick a strategy
through the registry at call time, so existing builders need no changes.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass, field
from typing import Any

from qiskit import QuantumCircuit, transpile

from backends import BackendSpec


@dataclass(frozen=True)
class CompiledCircuit:
    """The output of a compiler strategy."""

    circuit: QuantumCircuit
    two_qubit_count: int
    depth: int
    backend_name: str
    compiler_name: str
    metadata: dict[str, Any] = field(default_factory=dict)


class Compiler(ABC):
    """Strategy interface for compiling a logical circuit to a backend."""

    name: str

    @abstractmethod
    def compile(
        self,
        circuit: QuantumCircuit,
        spec: BackendSpec,
        optimization_level: int = 3,
        seed: int = 42,
    ) -> CompiledCircuit:
        """Compile ``circuit`` for the target ``spec`` and return the
        result with per-backend gate statistics attached."""


def _two_q(circ: QuantumCircuit) -> int:
    return sum(1 for op in circ.data if op.operation.num_qubits >= 2)


# -- Qiskit-only path ------------------------------------------------------


class QiskitCompiler(Compiler):
    name = "qiskit"

    def compile(
        self,
        circuit: QuantumCircuit,
        spec: BackendSpec,
        optimization_level: int = 3,
        seed: int = 42,
    ) -> CompiledCircuit:
        backend = spec.make_backend()
        out = transpile(
            circuit,
            backend,
            optimization_level=optimization_level,
            seed_transpiler=seed,
        )
        return CompiledCircuit(
            circuit=out,
            two_qubit_count=_two_q(out),
            depth=out.depth(),
            backend_name=spec.name,
            compiler_name=self.name,
            metadata={"optimization_level": optimization_level, "seed": seed},
        )


# -- TKET pipeline ---------------------------------------------------------


class TketCompiler(Compiler):
    """pytket pipeline followed by an optimisation-level-0 Qiskit pass to
    rejoin the Qiskit object model.

    Notes:
      * ``QuantumCircuit.decompose()`` is required once before
        ``qiskit_to_tk`` because pytket does not know about Qiskit's
        opaque ``QFTGate`` instruction.
      * ``DefaultMappingPass`` is given a TKET ``Architecture`` derived
        from the backend's coupling map, so the routing matches the
        physical device the same way Qiskit's transpile would.
    """

    name = "tket"

    def compile(
        self,
        circuit: QuantumCircuit,
        spec: BackendSpec,
        optimization_level: int = 3,
        seed: int = 42,
    ) -> CompiledCircuit:
        from pytket import OpType
        from pytket.architecture import Architecture
        from pytket.extensions.qiskit import qiskit_to_tk, tk_to_qiskit
        from pytket.passes import (
            AutoRebase,
            DecomposeBoxes,
            DefaultMappingPass,
            FullPeepholeOptimise,
            RemoveRedundancies,
            SequencePass,
        )

        target_optypes = _qiskit_gate_names_to_optypes(
            spec.one_qubit_gates + spec.two_qubit_gates
        ) | {OpType.Measure, OpType.Barrier}

        backend = spec.make_backend()
        coupling = list(backend.coupling_map.get_edges())
        arch = Architecture(coupling)

        # pytket cannot import QFTGate directly -- one decompose() flattens
        # it into controlled-phase + swap, which TKET handles.
        flat = circuit.decompose()
        tkc = qiskit_to_tk(flat)

        SequencePass(
            [
                DecomposeBoxes(),
                AutoRebase(target_optypes),
                FullPeepholeOptimise(),
                DefaultMappingPass(arch),
                AutoRebase(target_optypes),
                RemoveRedundancies(),
            ]
        ).apply(tkc)

        qc_back = tk_to_qiskit(tkc, replace_implicit_swaps=True)
        # Final harmonisation: Qiskit transpile at opt=0 to bind the layout
        # metadata for downstream samplers without reordering gates.
        out = transpile(qc_back, backend, optimization_level=0, seed_transpiler=seed)

        return CompiledCircuit(
            circuit=out,
            two_qubit_count=_two_q(out),
            depth=out.depth(),
            backend_name=spec.name,
            compiler_name=self.name,
            metadata={
                "optimization_level": optimization_level,
                "seed": seed,
                "tket_depth": tkc.depth(),
                "tket_2q": tkc.n_2qb_gates(),
            },
        )


def _qiskit_gate_names_to_optypes(names: tuple[str, ...]):
    from pytket import OpType

    table = {
        "cz": OpType.CZ,
        "ecr": OpType.ECR,
        "cx": OpType.CX,
        "rz": OpType.Rz,
        "sx": OpType.SX,
        "x": OpType.X,
        "h": OpType.H,
        "rzz": OpType.ZZPhase,
    }
    out = set()
    for n in names:
        if n not in table:
            raise ValueError(
                f"gate '{n}' has no pytket OpType mapping; extend "
                "_qiskit_gate_names_to_optypes if a new backend needs it"
            )
        out.add(table[n])
    return out


# -- Registry --------------------------------------------------------------


_REGISTRY: dict[str, Compiler] = {
    QiskitCompiler.name: QiskitCompiler(),
    TketCompiler.name: TketCompiler(),
}


def get_compiler(name: str) -> Compiler:
    if name not in _REGISTRY:
        raise ValueError(f"unknown compiler '{name}'; available: {sorted(_REGISTRY)}")
    return _REGISTRY[name]


def available_compilers() -> list[str]:
    return sorted(_REGISTRY)
