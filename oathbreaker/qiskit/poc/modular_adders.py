"""
Pluggable controlled-modular-add primitives for the Oathbreaker NISQ stack.

Each implementation realises the in-place operation

    |control, target, scratch=0>  ->  |control, (target + control*const) mod n_mod, scratch=0>

on a target register that already holds a value in [0, n_mod). The
implementations differ in their compiled hardware cost and their scratch-
qubit requirements; the Shor builder picks one through a simple registry.

Currently provided:

  - ``QFTBeauregardAdder``  -- the Beauregard 2003 mod-N adder built on top
    of QFT-basis phase rotations. Two-qubit cost is dominated by the QFT
    wrappers and grows as O(n^2) per add. Gold standard for correctness;
    used by the Oath-4 demo.

  - ``CDKMRippleAdder``  -- Beauregard structure with the QFT-basis adds
    replaced by Cuccaro-Draper-Kutin-Moulton ripple-carry adds (Qiskit's
    ``CDKMRippleCarryAdder``). Two-qubit cost grows linearly in n, which
    is the architectural change required to scale the Oath-4 demo up to
    Oath-8 / Oath-16 inside NISQ coherence budgets.

Both adders satisfy the same interface so the Shor builder can select one
without knowing the implementation.
"""

from __future__ import annotations

from abc import ABC, abstractmethod
from dataclasses import dataclass
from math import pi
from typing import Sequence

from qiskit import QuantumCircuit
from qiskit.circuit.library import CDKMRippleCarryAdder, QFTGate


@dataclass(frozen=True)
class AdderResources:
    """Scratch-qubit requirements for an adder implementation.

    ``flag_qubits``:        ancillae the caller must supply, returned to |0>.
    ``constant_qubits``:    extra register the adder will load with the
                            classical constant (n bits, also returned to |0>).
    ``target_overhead``:    extra qubits the target register must carry on
                            top of ceil(log2(n_mod)) (e.g. Beauregard's sign
                            bit).
    """

    flag_qubits: int
    constant_qubits: int
    target_overhead: int


class ControlledModularAdder(ABC):
    """Strategy interface for in-place controlled add-const mod n_mod."""

    name: str

    @abstractmethod
    def resources(self, n_bits: int) -> AdderResources:
        """Return the scratch-qubit requirements for an n_bits-target adder."""

    @abstractmethod
    def apply(
        self,
        qc: QuantumCircuit,
        const: int,
        n_mod: int,
        target: Sequence,
        control,
        flag: Sequence,
        const_reg: Sequence,
    ) -> None:
        """Append the controlled add to ``qc``. ``const_reg`` may be empty
        for adders that don't require a constant register."""


# -- Shared QFT-basis primitives used by Beauregard ------------------------


def _qft(qc: QuantumCircuit, qubits: Sequence) -> None:
    qc.append(QFTGate(len(qubits)), list(qubits))


def _iqft(qc: QuantumCircuit, qubits: Sequence) -> None:
    qc.append(QFTGate(len(qubits)).inverse(), list(qubits))


def _phi_add_const(
    qc: QuantumCircuit,
    const: int,
    qubits: Sequence,
    control=None,
) -> None:
    """Apply the Fourier-basis phase pattern that shifts the register by
    ``const`` (mod 2**n). Controlled if ``control`` is provided."""
    n = len(qubits)
    modulus = 1 << n
    const_mod = const % modulus
    if const_mod == 0:
        return
    for k, q in enumerate(qubits):
        angle = 2.0 * pi * const_mod * (1 << k) / modulus
        if control is None:
            qc.p(angle, q)
        else:
            qc.cp(angle, control, q)


# -- Beauregard QFT adder --------------------------------------------------


class QFTBeauregardAdder(ControlledModularAdder):
    """Beauregard 2003 mod-N adder. Two QFT/IQFT wrappers, three controlled
    add-const passes, one flag CX. Per-add 2q cost is O(n^2) on Heron."""

    name = "qft_beauregard"

    def resources(self, n_bits: int) -> AdderResources:
        return AdderResources(flag_qubits=1, constant_qubits=0, target_overhead=1)

    def apply(
        self,
        qc: QuantumCircuit,
        const: int,
        n_mod: int,
        target: Sequence,
        control,
        flag: Sequence,
        const_reg: Sequence,
    ) -> None:
        const = const % n_mod
        if const == 0:
            return
        t = list(target)
        f = flag[0]

        _qft(qc, t)
        _phi_add_const(qc, const, t, control=control)
        _phi_add_const(qc, -n_mod, t)
        _iqft(qc, t)

        qc.cx(t[-1], f)

        _qft(qc, t)
        _phi_add_const(qc, n_mod, t, control=f)
        _phi_add_const(qc, -const, t, control=control)
        _iqft(qc, t)

        qc.x(t[-1])
        qc.cx(t[-1], f)
        qc.x(t[-1])

        _qft(qc, t)
        _phi_add_const(qc, const, t, control=control)
        _iqft(qc, t)


# -- CDKM ripple-carry adder ----------------------------------------------


class CDKMRippleAdder(ControlledModularAdder):
    """Beauregard structure with the QFT-basis adders replaced by Cuccaro-
    Draper-Kutin-Moulton ripple-carry adders.

    The constant ``const`` is loaded into a separate n-bit register via X
    gates, fed into the CDKM adder, and unloaded after the add. Per-add 2q
    cost grows linearly in n, unlocking the scaling jump from Oath-4 to
    Oath-8 / Oath-16 within the NISQ coherence budget.
    """

    name = "cdkm_ripple"

    def resources(self, n_bits: int) -> AdderResources:
        # Beauregard's correctness requires a sign-bit ancilla on the target,
        # i.e. the modular arithmetic is performed mod 2**(n+1). CDKM adds
        # two equally-wide registers, so the constant register must also be
        # n+1 bits, and we need one CDKM cin-carry ancilla. The Beauregard
        # wraparound flag is the second flag qubit.
        return AdderResources(
            flag_qubits=2, constant_qubits=n_bits + 1, target_overhead=1
        )

    def _add_const(
        self,
        qc: QuantumCircuit,
        const: int,
        target: Sequence,
        const_reg: Sequence,
        carry,
        control=None,
        invert: bool = False,
    ) -> None:
        """Add `const` (or subtract if invert=True) to ``target`` via a CDKM
        ripple-carry adder. Optionally controlled by ``control``.

        Critical optimization: when ``control`` is given we do NOT wrap the
        whole CDKM gate in ``.control(1)`` -- Qiskit synthesises that as an
        expensive generic isometry. Instead we load ``control * const`` into
        the const register via CNOTs controlled on ``control``, do an
        unconditional CDKM add, then uncompute the load with the same
        CNOTs. This turns a quadratic-cost controlled isometry into a few
        CNOTs plus one unconditional ripple-carry adder.

        The full ``target`` register (n+1 bits = field bits + Beauregard
        sign bit) is operated on, so the arithmetic is mod 2**(n+1). The
        constant register is sized to match.
        """
        n = len(target)
        assert len(const_reg) == n, "const register must match target width"
        modulus = 1 << n
        c = (-const) % modulus if invert else const % modulus

        # Load c into const_reg. If ``control`` is provided, load it
        # conditionally via CNOTs; otherwise with X gates unconditionally.
        for k in range(n):
            if (c >> k) & 1:
                if control is None:
                    qc.x(const_reg[k])
                else:
                    qc.cx(control, const_reg[k])

        # Unconditional CDKM add: target += const_reg (mod 2**n).
        adder = CDKMRippleCarryAdder(n, kind="fixed").to_gate()
        qc.append(adder, [*const_reg, *target, carry])

        # Unload (same pattern, involutive).
        for k in range(n):
            if (c >> k) & 1:
                if control is None:
                    qc.x(const_reg[k])
                else:
                    qc.cx(control, const_reg[k])

    def apply(
        self,
        qc: QuantumCircuit,
        const: int,
        n_mod: int,
        target: Sequence,
        control,
        flag: Sequence,
        const_reg: Sequence,
    ) -> None:
        const = const % n_mod
        if const == 0:
            return
        # `flag[0]` = Beauregard wraparound flag.
        # `flag[1]` = CDKM cin/carry ancilla (returned to |0>).
        f = flag[0]
        carry = flag[1]
        sign = target[-1]  # Beauregard sign bit = top of the n+1-bit target

        # Step 1: target += control * const  (mod 2^(n+1))
        self._add_const(qc, const, target, const_reg, carry, control=control)
        # Step 2: target -= n_mod  (unconditional)
        self._add_const(qc, n_mod, target, const_reg, carry, invert=True)

        # Step 3: sign bit indicates wraparound -> flag
        qc.cx(sign, f)

        # Step 4: target += n_mod if flag
        self._add_const(qc, n_mod, target, const_reg, carry, control=f)
        # Step 5: undo step 1
        self._add_const(
            qc, const, target, const_reg, carry, invert=True, control=control
        )

        # Step 6: uncompute flag using inverted sign-bit sense
        qc.x(sign)
        qc.cx(sign, f)
        qc.x(sign)

        # Step 7: restore by adding controlled const
        self._add_const(qc, const, target, const_reg, carry, control=control)


# -- Registry --------------------------------------------------------------


_REGISTRY: dict[str, ControlledModularAdder] = {
    QFTBeauregardAdder.name: QFTBeauregardAdder(),
    CDKMRippleAdder.name: CDKMRippleAdder(),
}


def get_adder(name: str) -> ControlledModularAdder:
    if name not in _REGISTRY:
        raise ValueError(
            f"unknown adder method '{name}'. available: {sorted(_REGISTRY)}"
        )
    return _REGISTRY[name]


def available_methods() -> list[str]:
    return sorted(_REGISTRY)
