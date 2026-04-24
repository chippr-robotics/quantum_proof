"""
Oath-4 Shor ECDLP circuit for real IBM Quantum hardware.

Given a generator G of order n = 13 and target Q = [k]G on the Oath-4 curve,
this module builds a Qiskit circuit that recovers the scalar k via Shor's
period-finding algorithm for the discrete logarithm.

Circuit layout (NISQ, 12 data qubits + 8 classical bits):

    a_reg : 4 qubits  (exponent register for G, Hadamard-initialised)
    b_reg : 4 qubits  (exponent register for Q, Hadamard-initialised)
    idx   : 4 qubits  (group-index register, initialised to |0> = |infinity>)

For j = 0..3 we apply two controlled modular-addition permutations:

    C-add([2^j mod 13])         controlled on a_reg[j] -> idx
    C-add(index_of([2^j]Q))     controlled on b_reg[j] -> idx

The second permutation is constructed from Q and the classical EC addition
law without knowledge of k -- a user who only has (G, Q) and the curve can
build this circuit. Running it and measuring the exponent registers after
an inverse QFT yields samples (c1, c2) of the Shor spectrum, from which k
is recovered by a short classical lattice reduction.

Why this is the Oath-4 NISQ architectural proof:

  1. The controlled modular adders are the quantum analogue of the same
     reversible group-action circuit that the Oathbreaker Rust framework
     materialises at Oath-8/16/32 scale. Success on hardware at Oath-4
     validates the architecture end-to-end against a physical device.

  2. It closes the loop around the Groth16 / SP1 proof: the zkVM shows
     the classical circuit is correct; the hardware run shows it also
     executes correctly on noisy silicon. The two artefacts together
     cover the "correct in principle" and "correct in practice" claims.

  3. By compiling the group action through the Z/nZ isomorphism available
     only to a cyclic prime-order curve, the Oath-4 circuit fits under
     the NISQ gate budget of IBM Eagle / Heron processors with depth well
     below typical decoherence limits.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence

import numpy as np
from qiskit import QuantumCircuit, QuantumRegister, ClassicalRegister
from qiskit.circuit.library import QFTGate, UnitaryGate

from oath4 import (
    GROUP_ORDER,
    INDEX_BITS,
    Point,
    ec_add,
    ec_mul,
    GENERATOR,
    all_points,
    point_to_index,
)


REG_DIM: int = 1 << INDEX_BITS  # 16 basis states for a 4-qubit register


def _add_point_permutation(delta: Point) -> np.ndarray:
    """Build the permutation matrix for the map |idx> -> |idx'> where
    idx' is the index of (P_idx + delta) and delta is an EC point.

    The Oath-4 group has 13 elements encoded into a 4-qubit register, so
    basis states 13..15 are unused. We act as the identity on those states
    so the resulting 16x16 matrix is a genuine unitary and the circuit is
    well-defined even under transpiler noise.
    """
    points = all_points()  # index -> point
    perm = np.zeros((REG_DIM, REG_DIM), dtype=complex)
    for i in range(GROUP_ORDER):
        target_point = ec_add(points[i], delta)
        j = point_to_index(target_point)
        perm[j, i] = 1.0
    # identity on unused subspace
    for i in range(GROUP_ORDER, REG_DIM):
        perm[i, i] = 1.0
    return perm


def add_point_gate(delta: Point, label: str | None = None) -> UnitaryGate:
    """4-qubit unitary implementing |idx> -> |idx + delta>."""
    return UnitaryGate(_add_point_permutation(delta), label=label or f"+{delta}")


def controlled_add_point_gate(delta: Point, label: str | None = None):
    """5-qubit controlled version of add_point_gate.

    We build the 4-qubit permutation unitary and call `.control(1)` so
    Qiskit inserts the single control qubit with its own convention (LSB =
    qubits[0] in the append call). Caller passes `[*idx_reg, control]`.
    """
    base = add_point_gate(delta, label=label or f"+{delta}")
    return base.control(1, label=label or f"c+{delta}")


@dataclass
class Oath4Circuit:
    qc: QuantumCircuit
    a_reg: QuantumRegister
    b_reg: QuantumRegister
    idx_reg: QuantumRegister
    c_reg: ClassicalRegister


def build_oath4_shor_circuit(
    Q: Point,
    *,
    use_iqft_instruction: bool = True,
    measure: bool = True,
) -> Oath4Circuit:
    """Build the Oath-4 Shor ECDLP circuit for a given public point Q = [k]G.

    The circuit does NOT use k; it is constructed purely from G, Q and the
    curve equation. Running it on a backend and classically post-processing
    the measurement yields k.

    Parameters
    ----------
    Q : the public target point.
    use_iqft_instruction : if True, uses Qiskit's library QFT for the joint
        inverse QFT over the 8-qubit exponent register. Set False to expose
        the QFT gates for manual transpilation tuning.
    measure : if True, appends final measurements of the exponent registers.
    """
    a_reg = QuantumRegister(INDEX_BITS, name="a")
    b_reg = QuantumRegister(INDEX_BITS, name="b")
    idx_reg = QuantumRegister(INDEX_BITS, name="idx")
    c_reg = ClassicalRegister(2 * INDEX_BITS, name="meas")
    qc = QuantumCircuit(a_reg, b_reg, idx_reg, c_reg, name="oath4-shor")

    # Uniform superposition over the two exponent registers.
    qc.h(a_reg)
    qc.h(b_reg)

    # Controlled scalar additions on the index register.
    # For bit j of a_reg we add [2^j]G; for bit j of b_reg we add [2^j]Q.
    # Qiskit convention for Gate.control(1): the control qubit is the
    # least-significant qubit of the resulting gate, i.e. qubits[0] in the
    # append() call. Targets follow in LSB-first order.
    for j in range(INDEX_BITS):
        scalar_G = pow(2, j, GROUP_ORDER)
        jG = ec_mul(scalar_G, GENERATOR)
        gate_g = controlled_add_point_gate(jG, label=f"c+{scalar_G}G")
        qc.append(gate_g, [a_reg[j], *idx_reg])

        jQ = ec_mul(pow(2, j, GROUP_ORDER), Q)
        gate_q = controlled_add_point_gate(jQ, label=f"c+2^{j}Q")
        qc.append(gate_q, [b_reg[j], *idx_reg])

    # INDEPENDENT inverse QFTs on a_reg and b_reg. The a-register and
    # b-register are two separate periodic functions in Shor's ECDLP;
    # a joint IQFT_{2t} would mix cross-phases that destroy the structure.
    if use_iqft_instruction:
        qft_a = QFTGate(num_qubits=INDEX_BITS).inverse()
        qft_b = QFTGate(num_qubits=INDEX_BITS).inverse()
        qc.append(qft_a, list(a_reg))
        qc.append(qft_b, list(b_reg))
    else:
        _inverse_qft_inline(qc, list(a_reg))
        _inverse_qft_inline(qc, list(b_reg))

    if measure:
        qc.barrier()
        for i, qubit in enumerate([*a_reg, *b_reg]):
            qc.measure(qubit, c_reg[i])

    return Oath4Circuit(qc=qc, a_reg=a_reg, b_reg=b_reg, idx_reg=idx_reg, c_reg=c_reg)


def _inverse_qft_inline(qc: QuantumCircuit, qubits: Sequence) -> None:
    """In-place inverse QFT with final swaps."""
    n = len(qubits)
    for i in range(n // 2):
        qc.swap(qubits[i], qubits[n - 1 - i])
    for j in range(n):
        for m in range(j):
            qc.cp(-np.pi / (2 ** (j - m)), qubits[m], qubits[j])
        qc.h(qubits[j])


def recover_k_from_counts(counts: dict[str, int]) -> tuple[int, dict[int, int]]:
    """Classical post-processor: Shor-style recovery of k mod n.

    Each shot of the Oath-4 Shor circuit produces a measured pair (c1, c2)
    drawn from a spectrum peaked at c1 = s*N/n, c2 = s*k*N/n (mod N), where
    N = 2^t = 16 is the exponent-register size and s runs over 0..n-1.

    Dividing by N and multiplying by n gives integer approximations
        d1 = round(c1 * n / N)  ≈ s        mod n
        d2 = round(c2 * n / N)  ≈ s * k    mod n
    so that k = d2 * d1^{-1} (mod n) whenever d1 is invertible (i.e., d1 != 0
    which, since n is prime, is the only failure mode).

    We tally all invertible candidates per shot and return the mode.
    """
    t = INDEX_BITS
    N = 1 << t
    tally: dict[int, int] = {}
    skipped = 0
    for bitstring, freq in counts.items():
        clean = bitstring.replace(" ", "")
        assert len(clean) == 2 * t, clean
        # bitstring is MSB-first over classical bits. c_reg[0..t-1] <- a_reg
        # and c_reg[t..2t-1] <- b_reg, so:
        a_val = int(clean[-t:], 2)  # c1 (exponent of G)
        b_val = int(clean[:-t], 2)  # c2 (exponent of Q)
        d1 = round(a_val * GROUP_ORDER / N) % GROUP_ORDER
        d2 = round(b_val * GROUP_ORDER / N) % GROUP_ORDER
        if d1 == 0:
            skipped += freq
            continue
        k_guess = d2 * pow(d1, -1, GROUP_ORDER) % GROUP_ORDER
        tally[k_guess] = tally.get(k_guess, 0) + freq
    if not tally:
        raise ValueError(f"no usable shots (all had d1 == 0); skipped = {skipped}")
    best_k = max(tally, key=tally.get)
    return best_k, tally


if __name__ == "__main__":
    from oath4 import Instance

    inst = Instance.from_secret(7)
    bundle = build_oath4_shor_circuit(inst.Q)
    print(f"Oath-4 Shor ECDLP circuit for Q = {inst.Q} (secret k = {inst.k})")
    print(
        f"qubits = {bundle.qc.num_qubits}, "
        f"classical bits = {bundle.qc.num_clbits}, "
        f"pre-transpile depth = {bundle.qc.depth()}"
    )
    print(f"instruction count (unrolled one level): {sum(1 for _ in bundle.qc.data)}")
