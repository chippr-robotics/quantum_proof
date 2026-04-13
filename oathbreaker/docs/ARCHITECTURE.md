# Oathbreaker Architecture

This document describes how the Oathbreaker circuit is structured, how data
flows through the system, and where computational cost concentrates. All
diagrams use [Mermaid](https://mermaid.js.org/) syntax and render on GitHub.

---

## Crate Dependency Graph

The workspace is organized into seven crates with a strict bottom-up
dependency chain. No circular dependencies exist.

```mermaid
graph BT
    GF["goldilocks-field<br/><i>GF(p) arithmetic</i>"]
    EC["ec-goldilocks<br/><i>EC point operations</i>"]
    RA["reversible-arithmetic<br/><i>Reversible gates + circuits</i>"]
    GAC["group-action-circuit<br/><i>Circuit assembly</i>"]
    BM["benchmark<br/><i>Resource counting + projections</i>"]
    SP1H["sp1-host<br/><i>ZK proof host</i>"]
    SP1P["sp1-program<br/><i>ZK proof guest</i>"]

    EC --> GF
    RA --> GF
    RA --> EC
    GAC --> GF
    GAC --> EC
    GAC --> RA
    BM --> GAC
    BM --> RA
    BM --> EC
    BM --> GF
    SP1H --> GAC
    SP1H --> EC
    SP1P --> EC

    style GF fill:#e8f5e9,stroke:#2e7d32
    style EC fill:#e3f2fd,stroke:#1565c0
    style RA fill:#fff3e0,stroke:#e65100
    style GAC fill:#fce4ec,stroke:#c62828
    style BM fill:#f3e5f5,stroke:#6a1b9a
```

**What each crate does:**

| Crate | Role | Key types |
|-------|------|-----------|
| `goldilocks-field` | Modular arithmetic over p = 2^64 - 2^32 + 1 | `GoldilocksField` |
| `ec-goldilocks` | Classical elliptic curve operations + ECDLP solvers | `AffinePoint`, `JacobianPoint`, `CurveParams` |
| `reversible-arithmetic` | Reversible gate primitives and arithmetic circuits | `Gate`, `CuccaroAdder`, `KaratsubaMultiplier`, `BinaryGcdInverter` |
| `group-action-circuit` | Complete Shor's circuit: group-action + QFT + measurement + recovery | `ShorsEcdlp`, `GroupActionCircuit`, `Qft`, `QuantumGate` |
| `benchmark` | Measures resources, projects to 256-bit, compares to literature | `ScalingProjection`, `CostAttribution` |

---

## Circuit Construction Pipeline

Building a circuit follows a top-down assembly process. The benchmark
binary drives construction; the circuit builder wires together scalar
multiplications, an inversion, and affine recovery.

```mermaid
flowchart TD
    subgraph Benchmark
        B1[Load curve params<br/>from sage/*.json]
        B2[Select window size w]
        B3["Build GroupActionCircuit<br/>(Jacobian variant)"]
        B4[Print resource table<br/>+ cost attribution]
        B5["Project to 256-bit<br/>(Karatsuba / empirical)"]
    end

    subgraph Circuit Assembly
        C1["Allocate primary registers<br/>reg_a(n) + reg_b(n) + X(n) + Y(n) + Z(n) = 5n qubits"]
        C2["Windowed scalar mul [a]G<br/>(n/w windows)"]
        C3["Windowed scalar mul [b]Q<br/>(n/w windows, same accumulator)"]
        C4["Final inversion<br/>Binary GCD: Z → Z⁻¹"]
        C5["Affine recovery<br/>x = X·Z⁻², y = Y·Z⁻³"]
    end

    B1 --> B2 --> B3
    B3 --> C1 --> C2 --> C3 --> C4 --> C5
    C5 --> B4 --> B5

    style C2 fill:#fff3e0,stroke:#e65100
    style C3 fill:#fff3e0,stroke:#e65100
    style C4 fill:#e3f2fd,stroke:#1565c0
    style C5 fill:#e8f5e9,stroke:#2e7d32
```

---

## Windowed Scalar Multiplication (Core Loop)

This is where >97% of the Toffoli gates are generated. Each window
iteration has four phases. The doubling phase dominates at ~80% of
total circuit cost.

```mermaid
flowchart TD
    START([Start: window i of n/w]) --> DBL

    subgraph DOUBLING ["Phase 1: Doublings (80% of Toffoli)"]
        DBL["Repeat w times:<br/>Jacobian point doubling<br/>(6 squarings + 3 multiplications)"]
        SWAP1["CNOT-swap result<br/>back to accumulator"]
        DBL --> SWAP1
    end

    SWAP1 --> QROM

    subgraph QROM_PHASE ["Phase 2: QROM Lookup (0.1% of Toffoli)"]
        QROM["One-hot decode<br/>w scalar bits → 2^w selection register"]
        LOAD["CNOT-load selected<br/>precomputed table entry"]
        UNDECODE["Reverse one-hot decode<br/>(clean selection register)"]
        QROM --> LOAD --> UNDECODE
    end

    UNDECODE --> ADD

    subgraph ADDITION ["Phase 3: Mixed Addition (17% of Toffoli)"]
        ADD["Jacobian mixed addition<br/>accumulator += table entry<br/>(3 squarings + 8 multiplications)"]
        SWAP2["CNOT-swap result<br/>back to accumulator"]
        ADD --> SWAP2
    end

    SWAP2 --> UNCOMPUTE

    subgraph CLEANUP ["Phase 4: QROM Uncompute"]
        UNCOMPUTE["Replay QROM gates in reverse<br/>to clean lookup registers"]
    end

    UNCOMPUTE --> NEXT{More windows?}
    NEXT -- Yes --> START
    NEXT -- No --> DONE([Done: accumulator holds result])

    style DOUBLING fill:#fff3e0,stroke:#e65100
    style QROM_PHASE fill:#e8f5e9,stroke:#2e7d32
    style ADDITION fill:#fce4ec,stroke:#c62828
    style CLEANUP fill:#f3e5f5,stroke:#6a1b9a
```

---

## Arithmetic Stack

Every EC operation decomposes into field multiplications, which
decompose into gate-level primitives. The Karatsuba multiplier
sits at the center of this stack.

```mermaid
graph TD
    subgraph "EC Operations"
        DOUBLE["Jacobian Doubling<br/>6S + 3M per call"]
        MADD["Jacobian Mixed Addition<br/>3S + 8M per call"]
        INV["Binary GCD Inversion<br/>O(n²) Toffoli, no multiplications"]
    end

    subgraph "Field Arithmetic"
        KMUL["Karatsuba Multiplier<br/>O(n^1.585) Toffoli per multiply"]
        KSQ["Karatsuba Squarer<br/>~50% fewer Toffoli via symmetry"]
        CSUB["Cuccaro Subtraction<br/>O(n) Toffoli per subtract"]
        CADD["Cuccaro Addition<br/>O(n) Toffoli per add"]
    end

    subgraph "Gate Primitives"
        TOF["Toffoli (CCNOT)<br/>universal reversible gate"]
        CNOT["CNOT<br/>controlled-NOT"]
        NOT["NOT (Pauli-X)<br/>bit flip"]
    end

    DOUBLE --> KMUL
    DOUBLE --> KSQ
    DOUBLE --> CSUB
    MADD --> KMUL
    MADD --> KSQ
    MADD --> CSUB
    INV --> CADD
    INV --> CSUB

    KMUL --> TOF
    KMUL --> CNOT
    KSQ --> TOF
    KSQ --> CNOT
    CSUB --> TOF
    CSUB --> CNOT
    CADD --> TOF
    CADD --> CNOT

    style KMUL fill:#fff3e0,stroke:#e65100
    style KSQ fill:#fff3e0,stroke:#e65100
    style TOF fill:#fce4ec,stroke:#c62828
```

### Karatsuba Multiplication (Recursive Structure)

The Karatsuba multiplier recursively splits n-bit operands into halves,
performing 3 sub-multiplications instead of 4. It falls back to schoolbook
at n <= 8.

```mermaid
flowchart TD
    INPUT["a × b (n-bit operands)"] --> CHECK{n <= 8?}
    CHECK -- Yes --> SCHOOL["Schoolbook<br/>O(n²) partial products"]
    CHECK -- No --> SPLIT["Split: a = a_hi·2^(n/2) + a_lo<br/>b = b_hi·2^(n/2) + b_lo"]

    SPLIT --> Z0["z0 = a_lo × b_lo<br/>(recursive, n/2 bits)"]
    SPLIT --> Z2["z2 = a_hi × b_hi<br/>(recursive, n/2 bits)"]
    SPLIT --> SUMS["sa = a_lo + a_hi<br/>sb = b_lo + b_hi<br/>(Cuccaro with carry-out)"]

    SUMS --> Z1["z1_full = sa × sb<br/>(recursive, n/2+1 bits)"]
    Z1 --> SUB["z1 = z1_full - z0 - z2<br/>(Cuccaro subtraction)"]
    Z0 --> COMBINE
    Z2 --> COMBINE
    SUB --> COMBINE["Combine:<br/>result = z0 + z1·2^(n/2) + z2·2^n"]
    COMBINE --> REDUCE["Goldilocks modular reduction<br/>fold high bits using 2^n ≡ 2^(n/2) - 1"]
    REDUCE --> COPY["CNOT copy to output"]
    COPY --> BENNETT["Bennett uncomputation<br/>(reverse all forward gates)"]

    SCHOOL --> REDUCE

    style SPLIT fill:#e3f2fd,stroke:#1565c0
    style COMBINE fill:#e8f5e9,stroke:#2e7d32
    style BENNETT fill:#f3e5f5,stroke:#6a1b9a
```

When squaring (a × a), the `is_square` flag propagates through the
recursion. At the base case, `schoolbook_integer_square` exploits
cross-term symmetry: each a[i]·a[j] pair is computed once (not twice),
saving ~50% of partial-product Toffoli.

---

## QROM One-Hot Decode

The QROM (Quantum Read-Only Memory) loads a classically precomputed
table entry into quantum registers, controlled by w scalar bits.

```mermaid
flowchart LR
    subgraph "Scalar register"
        S["w bits:<br/>s[0], s[1], ..., s[w-1]"]
    end

    subgraph "One-hot decode"
        OH["2^w selection qubits<br/>exactly one bit set"]
    end

    subgraph "Lookup registers"
        LX["lookup_x (n bits)"]
        LY["lookup_y (n bits)"]
    end

    S -->|"O(2^w) Toffoli<br/>binary→one-hot"| OH
    OH -->|"O(2^w × n) CNOT<br/>load table entry"| LX
    OH -->|"O(2^w × n) CNOT<br/>load table entry"| LY
    OH -->|"O(2^w) Toffoli<br/>reverse decode"| S
```

The decode algorithm processes scalar bits one at a time, splitting
each active selection entry into two branches (bit=0 and bit=1) via
Toffoli + CNOT pairs. After all w bits are processed, exactly one
of the 2^w one-hot qubits is set.

---

## Cost Attribution (Oath-32, w=8)

This is the measured Toffoli breakdown from the benchmark. It shows
where optimization effort should focus.

```mermaid
pie title Toffoli Cost Attribution (Oath-32)
    "Doublings (80.2%)" : 80.2
    "Mixed Additions (17.1%)" : 17.1
    "Inversion — BGCD (1.9%)" : 1.9
    "Affine Recovery (0.7%)" : 0.7
    "QROM (0.1%)" : 0.1
```

**Reading the chart:** Doublings dominate because each window iteration
performs w=8 doublings but only 1 addition. Each doubling costs
6S + 3M = 9 multiplication-equivalents; each addition costs 3S + 8M = 11.
With 8 doublings per addition, doublings account for 72/(72+11) = 87%
of the EC arithmetic, consistent with the measured 80%/17% split.

---

## Bennett's Compute-Copy-Uncompute Pattern

Every reversible subroutine in the circuit uses Bennett's pattern to
clean ancilla qubits. This is the dominant source of the ~2x gate
overhead compared to measurement-based approaches.

```mermaid
sequenceDiagram
    participant Input as Input registers
    participant Work as Workspace (ancilla)
    participant Out as Output register

    Note over Input,Out: Forward computation
    Input->>Work: Compute intermediate values
    Note over Work: Workspace now dirty

    Note over Input,Out: Copy result
    Work->>Out: CNOT copy result to output

    Note over Input,Out: Reverse computation (Bennett)
    Work->>Input: Run all forward gates in reverse
    Note over Work: Workspace returned to |0⟩

    Note over Out: Output retains the result
    Note over Work: Ancilla clean — can be reused
```

**Impact on cost:** Every multiplication, squaring, and inversion pays
this 2x overhead. The forward gates are collected into a `Vec<Gate>`,
the result is copied, then the same gates are replayed in reverse.
This is why measurement-based uncomputation (which avoids the reverse
pass) would roughly halve the total gate count.

---

## Binary GCD Inversion

The Binary GCD inverter replaced Fermat's method (125 multiplications)
with an O(n²) algorithm that uses only additions, subtractions, and shifts.

```mermaid
flowchart TD
    INIT["Initialize:<br/>u = p, v = a, r = 0, s = 1"]

    subgraph PHASE1 ["Phase 1: Extended Binary GCD (2n iterations)"]
        COND["Compute swap condition<br/>from u[0], v[0], u > v"]
        CSWAP["Conditional swap (u,v) and (r,s)<br/>3n Toffoli per swap"]
        ODD{"Both u,v odd?"}
        SUBTRACT["u -= v (Cuccaro subtract)<br/>r += s (Cuccaro add)"]
        SHIFT["Right-shift u by 1<br/>Left-shift s by 1"]
        UNCSWAP["Reverse conditional swap"]
        UNSWAP_COND["Uncompute swap condition"]

        COND --> CSWAP --> ODD
        ODD -- Yes --> SUBTRACT --> SHIFT
        ODD -- No --> SHIFT
        SHIFT --> UNCSWAP --> UNSWAP_COND
    end

    subgraph PHASE2 ["Phase 2: Montgomery Correction (2n halvings)"]
        HALF{"r odd?"}
        ADDP["r += p (conditional Cuccaro add)"]
        RSHIFT["Right-shift r by 1"]

        HALF -- Yes --> ADDP --> RSHIFT
        HALF -- No --> RSHIFT
    end

    INIT --> PHASE1
    PHASE1 -->|"r = a⁻¹ · 2^k mod p"| PHASE2
    PHASE2 -->|"r = a⁻¹ mod p"| COPY["CNOT copy r to output"]
    COPY --> BENNETT["Bennett: reverse Phase 1 + Phase 2"]
    BENNETT --> DONE([Clean workspace, output holds a⁻¹])

    style PHASE1 fill:#e3f2fd,stroke:#1565c0
    style PHASE2 fill:#e8f5e9,stroke:#2e7d32
    style BENNETT fill:#f3e5f5,stroke:#6a1b9a
```

**Cost comparison at Oath-32 (n=32):**

| Inverter | Toffoli | Share of total circuit |
|----------|---------|----------------------|
| Fermat (old) | ~960K | ~15% |
| Binary GCD (current) | 107K | 1.9% |

---

## Scaling Projections

The benchmark reports three projection models from measured small-tier
results to 256-bit ECDLP estimates.

```mermaid
graph LR
    M8["Oath-8<br/>(measured)"] --> M16["Oath-16<br/>(measured)"]
    M16 --> M32["Oath-32<br/>(measured)"]
    M32 --> P64["Oath-64<br/>(projected)"]
    P64 --> P256["Oath-256<br/>(projected)"]

    M32 -.->|"Empirical exponent<br/>fit from 16→32 ratio"| EMP["Empirical O(n^2.51)"]
    M32 -.->|"Karatsuba theory"| KARA["Karatsuba O(n^2.585)"]
    M32 -.->|"Schoolbook theory<br/>(legacy comparison)"| SCHOOL["Schoolbook O(n^3)"]

    EMP --> P256_E["1.2B Toffoli"]
    KARA --> P256_K["1.4B Toffoli"]
    SCHOOL --> P256_S["3.4B Toffoli"]

    style M8 fill:#e8f5e9
    style M16 fill:#e8f5e9
    style M32 fill:#e8f5e9
    style P64 fill:#fff3e0
    style P256 fill:#fce4ec
```

---

## File Map

Quick reference for navigating the codebase by concern.

```
oathbreaker/
├── crates/
│   ├── goldilocks-field/src/
│   │   ├── field.rs            # GoldilocksField: add, sub, mul, inverse, pow
│   │   └── constants.rs        # GOLDILOCKS_PRIME, P_MINUS_TWO
│   │
│   ├── ec-goldilocks/src/
│   │   ├── curve.rs            # CurveParams, AffinePoint, JacobianPoint
│   │   ├── point_ops.rs        # add, double, scalar_mul (both coord systems)
│   │   └── ecdlp.rs            # Pollard's rho, BSGS solvers
│   │
│   ├── reversible-arithmetic/src/
│   │   ├── gates.rs            # Gate enum: NOT, CNOT, Toffoli
│   │   ├── adder.rs            # CuccaroAdder (plain + modular)
│   │   ├── multiplier.rs       # Schoolbook, Karatsuba, Squarer, cuccaro_subtract
│   │   ├── inverter.rs         # FermatInverter, BinaryGcdInverter
│   │   ├── ec_add_jacobian.rs  # Reversible Jacobian mixed addition (3S+8M)
│   │   ├── ec_double_jacobian.rs # Reversible Jacobian doubling (6S+3M)
│   │   ├── ancilla.rs          # AncillaPool, UncomputeStrategy
│   │   └── resource_counter.rs # Toffoli/CNOT/NOT tracking
│   │
│   ├── group-action-circuit/src/
│   │   ├── double_scalar.rs    # GroupActionCircuit builder, CostAttribution
│   │   ├── scalar_mul_jacobian.rs # Windowed scalar mul + QROM one-hot decode
│   │   ├── precompute.rs       # Classical QROM table generation
│   │   ├── quantum_gate.rs     # Extended gate enum (Hadamard, CR, Swap, Measure)
│   │   ├── qft.rs              # QFT/inverse QFT gate generation + classical DFT sim
│   │   ├── qft_stub.rs         # QFT resource estimates (backward compat)
│   │   ├── measurement.rs      # Shor measurement outcome simulation
│   │   ├── continued_fraction.rs # CF expansion + ECDLP secret recovery
│   │   ├── shor.rs             # End-to-end Shor's ECDLP pipeline (ShorsEcdlp)
│   │   └── export.rs           # OpenQASM 3.0 export (full Shor circuit)
│   │
│   └── benchmark/src/
│       ├── main.rs             # Benchmark orchestration, window sweep
│       ├── scaling.rs          # Karatsuba/schoolbook/empirical projections
│       ├── comparison.rs       # Prior work table (Litinski, Google, etc.)
│       └── oath_tiers.rs       # Oath-8/16/32/64 tier definitions
│
├── docs/
│   ├── ARCHITECTURE.md         # This file
│   ├── CIRCUIT_ARCHITECTURE.md # Register layouts, gate decomposition
│   ├── COMPARISON.md           # Comparison to prior work
│   ├── LIMITATIONS.md          # Scope and known limitations
│   ├── BENCHMARKING.md         # Oathbreaker Scale specification
│   └── VERIFICATION.md         # Testing and verification layers
│
└── sage/                       # SageMath curve generation scripts
