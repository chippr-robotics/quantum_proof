"""
Generic Oath-N hardware-optimized Shor ECDLP circuit builder.

Generalises ``oath4_optimized.py`` to any Oath tier (4, 8, 16, 32, ...)
by loading curve parameters from ``../sage/oath{N}_params.json`` via
``OathCurve.load_tier`` and sizing the registers accordingly.

The controlled modular adder is pluggable through ``modular_adders`` and
the ECDLP recovery follows the same two-register Shor spectrum analysis
as at Oath-4, so the recovered scalar post-processing is tier-agnostic.

Note on scaling: the NISQ compilation trick used here -- mapping each
controlled-add-point operation onto a controlled-add-const-mod-N via the
cyclic-group isomorphism E(F_p) ~= Z/nZ -- requires a *classical* lookup
``point_to_index`` to size each add. That scan is polynomial in n and
therefore tractable for Oath-4, Oath-8 and Oath-16 (up to ~65K group
elements). For Oath-32 and beyond the scan is infeasible and the real
Oathbreaker route is the full reversible EC-arithmetic circuit built by
the Rust ``crates/group-action-circuit`` crate. This module therefore
targets Oath-4 / Oath-8 / Oath-16 for the NISQ-executable regime.
"""

from __future__ import annotations

from dataclasses import dataclass

from qiskit import ClassicalRegister, QuantumCircuit, QuantumRegister
from qiskit.circuit.library import QFTGate

from modular_adders import ControlledModularAdder, available_methods, get_adder
from oath_curve import OathCurve, Point


@dataclass
class OathNCircuit:
    """A compiled Oath-N Shor ECDLP circuit and its register handles."""

    qc: QuantumCircuit
    a_reg: QuantumRegister
    b_reg: QuantumRegister
    idx_reg: QuantumRegister
    c_reg: ClassicalRegister
    tier: int
    adder_method: str


def build_oathN_shor_circuit(
    curve: OathCurve,
    Q: Point,
    *,
    measure: bool = True,
    adder_method: str = "qft_beauregard",
) -> OathNCircuit:
    """Build the hardware-optimized Shor ECDLP circuit for any Oath tier.

    Parameters
    ----------
    curve : OathCurve
        The Oath-N curve (loaded via ``OathCurve.load_tier``).
    Q : Point
        The public target ``Q = [k]G`` on ``curve``.
    adder_method : str
        Name of the controlled modular adder implementation; see
        ``modular_adders.available_methods``. Defaults to the QFT-basis
        Beauregard adder used at Oath-4; switch to ``cdkm_ripple`` for
        the linear-in-n ripple-carry variant required at Oath-8+.
    measure : bool
        If True, append final measurements of the two exponent registers.
    """
    if adder_method not in available_methods():
        raise ValueError(
            f"adder_method must be one of {available_methods()}, got {adder_method!r}"
        )
    adder: ControlledModularAdder = get_adder(adder_method)

    n = curve.index_bits
    res = adder.resources(n)

    a_reg = QuantumRegister(n, name="a")
    b_reg = QuantumRegister(n, name="b")
    idx_reg = QuantumRegister(n + res.target_overhead, name="idx")
    flag = QuantumRegister(res.flag_qubits, name="flag")
    const_reg = (
        QuantumRegister(res.constant_qubits, name="const")
        if res.constant_qubits
        else None
    )
    c_reg = ClassicalRegister(2 * n, name="meas")

    if const_reg is not None:
        qc = QuantumCircuit(
            a_reg,
            b_reg,
            idx_reg,
            flag,
            const_reg,
            c_reg,
            name=f"oath{curve.tier}-shor-{adder_method}",
        )
    else:
        qc = QuantumCircuit(
            a_reg,
            b_reg,
            idx_reg,
            flag,
            c_reg,
            name=f"oath{curve.tier}-shor-{adder_method}",
        )

    qc.h(a_reg)
    qc.h(b_reg)

    idx_qubits = list(idx_reg)
    flag_qubits = list(flag)
    const_qubits = list(const_reg) if const_reg is not None else []

    n_mod = curve.order
    for j in range(n):
        scalar = pow(2, j, n_mod)
        # Controlled add [2^j]G, which maps |idx> -> |(idx + scalar) mod n_mod>.
        adder.apply(qc, scalar, n_mod, idx_qubits, a_reg[j], flag_qubits, const_qubits)

        # Controlled add [2^j]Q. Its index under the isomorphism is the
        # classical discrete log of [2^j]Q, which the circuit BUILDER (not
        # the attacker) computes via a linear scan. See module docstring
        # for the scaling-limit caveat.
        point = curve.ec_mul(scalar, Q)
        const_q = curve.point_to_index(point)
        adder.apply(qc, const_q, n_mod, idx_qubits, b_reg[j], flag_qubits, const_qubits)

    qc.append(QFTGate(n).inverse(), list(a_reg))
    qc.append(QFTGate(n).inverse(), list(b_reg))

    if measure:
        qc.barrier()
        for i, qubit in enumerate([*a_reg, *b_reg]):
            qc.measure(qubit, c_reg[i])

    return OathNCircuit(
        qc=qc,
        a_reg=a_reg,
        b_reg=b_reg,
        idx_reg=idx_reg,
        c_reg=c_reg,
        tier=curve.tier,
        adder_method=adder_method,
    )


def recover_k_from_counts(
    counts: dict[str, int], curve: OathCurve
) -> tuple[int, dict[int, int]]:
    """Tier-agnostic Shor post-processor.

    Each shot produces a pair (c1, c2) drawn from a spectrum peaked at
    (s*N/n, s*k*N/n mod N), where N = 2^t is the exponent-register
    width and n = curve.order. We tally invertible candidates and
    return the mode.
    """
    n = curve.order
    t = curve.index_bits
    N = 1 << t
    tally: dict[int, int] = {}
    for bitstring, freq in counts.items():
        clean = bitstring.replace(" ", "")
        if len(clean) != 2 * t:
            raise ValueError(f"expected {2 * t}-bit classical register; got {clean!r}")
        a_val = int(clean[-t:], 2)
        b_val = int(clean[:-t], 2)
        d1 = round(a_val * n / N) % n
        d2 = round(b_val * n / N) % n
        if d1 == 0:
            continue
        k_guess = d2 * pow(d1, -1, n) % n
        if k_guess == 0:
            continue
        tally[k_guess] = tally.get(k_guess, 0) + freq
    if not tally:
        raise ValueError("no usable shots for k recovery")
    return max(tally, key=tally.get), tally


if __name__ == "__main__":
    from oath_curve import OathInstance

    for tier in (4, 8):
        curve = OathCurve.load_tier(tier)
        inst = OathInstance.from_secret(curve, 7 % curve.order)
        for method in available_methods():
            bundle = build_oathN_shor_circuit(curve, inst.Q, adder_method=method)
            print(
                f"Oath-{tier} / {method:<15s}  qubits={bundle.qc.num_qubits:3d}  "
                f"depth={bundle.qc.depth():4d}  "
                f"instructions={sum(1 for _ in bundle.qc.data):5d}"
            )
