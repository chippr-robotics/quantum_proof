"""
Hardware-optimized Oath-4 Shor ECDLP circuit.

The baseline circuit in ``oath4_circuit.py`` hands each controlled modular-
addition permutation to Qiskit as an opaque 5-qubit ``UnitaryGate.control(1)``.
The default isometry synthesis blows that up to ~3 600 ECR/CZ gates on current
IBM backends -- far above NISQ coherence budgets.

This module replaces those opaque unitaries with explicit modular adders
provided by ``modular_adders.py``. The choice of adder is pluggable so the
same Shor builder can drive either the QFT-basis Beauregard adder (used at
Oath-4) or the CDKM ripple-carry adder (the next architectural step toward
Oath-8 / Oath-16 NISQ feasibility).
"""

from __future__ import annotations

from qiskit import ClassicalRegister, QuantumCircuit, QuantumRegister
from qiskit.circuit.library import QFTGate

from oath4 import GENERATOR, GROUP_ORDER, INDEX_BITS, Point, ec_mul, point_to_index
from oath4_circuit import Oath4Circuit
from modular_adders import (
    ControlledModularAdder,
    available_methods,
    get_adder,
)


def _scalar_index_for(point: Point) -> int:
    """Return the integer k in 0..n-1 such that [k]G = point."""
    return point_to_index(point)


def build_oath4_shor_circuit_optimized(
    Q: Point,
    *,
    measure: bool = True,
    adder_method: str = "qft_beauregard",
) -> Oath4Circuit:
    """Hardware-oriented Oath-4 Shor circuit with a pluggable modular adder.

    ``adder_method`` selects the controlled-add-const-mod-N implementation
    from ``modular_adders``. The default 'qft_beauregard' is the same
    Beauregard QFT adder used in the original Oath-4 demo. 'cdkm_ripple'
    swaps in the CDKM ripple-carry adder, which scales linearly with bit
    count and is the architectural step required for Oath-8 / Oath-16.

    Output bitstring convention, classical recovery helper and overall
    correctness are identical to the baseline, so
    ``recover_k_from_counts`` from ``oath4_circuit`` applies unchanged.
    """
    if adder_method not in available_methods():
        raise ValueError(
            f"adder_method must be one of {available_methods()}, got {adder_method!r}"
        )
    adder: ControlledModularAdder = get_adder(adder_method)
    res = adder.resources(INDEX_BITS)

    a_reg = QuantumRegister(INDEX_BITS, name="a")
    b_reg = QuantumRegister(INDEX_BITS, name="b")
    idx_reg = QuantumRegister(INDEX_BITS + res.target_overhead, name="idx")
    flag = QuantumRegister(res.flag_qubits, name="flag")
    const_reg = (
        QuantumRegister(res.constant_qubits, name="const")
        if res.constant_qubits
        else None
    )
    c_reg = ClassicalRegister(2 * INDEX_BITS, name="meas")

    if const_reg is not None:
        qc = QuantumCircuit(
            a_reg,
            b_reg,
            idx_reg,
            flag,
            const_reg,
            c_reg,
            name=f"oath4-shor-{adder_method}",
        )
    else:
        qc = QuantumCircuit(
            a_reg,
            b_reg,
            idx_reg,
            flag,
            c_reg,
            name=f"oath4-shor-{adder_method}",
        )

    qc.h(a_reg)
    qc.h(b_reg)

    idx_qubits = list(idx_reg)
    flag_qubits = list(flag)
    const_qubits = list(const_reg) if const_reg is not None else []

    for j in range(INDEX_BITS):
        scalar_G = pow(2, j, GROUP_ORDER)
        const_g = _scalar_index_for(ec_mul(scalar_G, GENERATOR))
        adder.apply(
            qc, const_g, GROUP_ORDER, idx_qubits, a_reg[j], flag_qubits, const_qubits
        )

        const_q = _scalar_index_for(ec_mul(scalar_G, Q))
        adder.apply(
            qc, const_q, GROUP_ORDER, idx_qubits, b_reg[j], flag_qubits, const_qubits
        )

    qc.append(QFTGate(INDEX_BITS).inverse(), list(a_reg))
    qc.append(QFTGate(INDEX_BITS).inverse(), list(b_reg))

    if measure:
        qc.barrier()
        for i, qubit in enumerate([*a_reg, *b_reg]):
            qc.measure(qubit, c_reg[i])

    return Oath4Circuit(qc=qc, a_reg=a_reg, b_reg=b_reg, idx_reg=idx_reg, c_reg=c_reg)


if __name__ == "__main__":
    from oath4 import Instance

    inst = Instance.from_secret(7)
    for method in available_methods():
        bundle = build_oath4_shor_circuit_optimized(inst.Q, adder_method=method)
        print(
            f"[{method}]  qubits={bundle.qc.num_qubits}  "
            f"depth={bundle.qc.depth()}  "
            f"high-level instructions={sum(1 for _ in bundle.qc.data)}"
        )
