# Zero-Knowledge Proof Guide

## Overview

The Oathbreaker ZKP proves that a quantum circuit of specific resource counts
correctly computes the coherent double-scalar group-action map `[a]G + [b]Q`
on an Oath-N elliptic curve. The proof is generated using the **SP1 zkVM**
(Succinct Proof System 1) and compressed into a **Groth16 SNARK** for
on-chain verification.

This follows the same approach as Google Quantum AI's
[zkp_ecc](https://github.com/tanujkhattar/zkp_ecc) project, which uses SP1
to prove correct execution of quantum circuit simulations for secp256k1. The
Oathbreaker project applies the same pattern to the Oath curve family
(prime-order short-Weierstrass curves over word-sized primes; Oath-64 uses
the canonical Goldilocks prime).

### What the proof attests

1. A reversible circuit was constructed with specific resource counts
   (logical qubits, Toffoli gates, CNOT gates, circuit depth)
2. The circuit correctly computes `[a]G + [b]Q` on N randomly-sampled
   test inputs, matching the classical double-scalar multiplication reference
3. A SHA-256 hash of the full `CircuitSummary` is committed, binding the
   proof to a specific circuit structure

### What the proof does NOT attest

- Correctness over quantum superpositions (assumed via reversibility)
- That Shor's algorithm succeeds on quantum hardware

Note: The QFT + measurement + classical recovery pipeline is now fully implemented
and tested (54 tests in `group-action-circuit`), but the ZK proof covers only the
group-action map portion, not the QFT/measurement stages.

## Architecture

### Host / Guest Split

SP1 uses a two-program architecture:

```
┌──────────────────────────────────────────┐
│              SP1 Host (sp1-host)         │
│                                          │
│  1. Load curve params from JSON          │
│  2. Generate random test cases           │
│  3. Create ProofInput                    │
│  4. Send to guest via SP1Stdin           │
│  5. Receive ProofOutput (public values)  │
│  6. Write artifacts to proofs/           │
└────────────────┬─────────────────────────┘
                 │ SP1Stdin (ProofInput)
                 ▼
┌──────────────────────────────────────────┐
│           SP1 Guest (sp1-program)        │
│         [Runs inside SP1 zkVM]           │
│                                          │
│  1. Read ProofInput from host            │
│  2. Build group-action circuit           │
│  3. Compute SHA-256 of CircuitSummary    │
│  4. Verify test cases against reference  │
│  5. Commit ProofOutput (public values)   │
│                                          │
│  Every instruction is cryptographically  │
│  proven via STARK → Groth16 compression  │
└──────────────────────────────────────────┘
```

The **guest program** is compiled to RISC-V machine code and executed inside
the SP1 zkVM. Every instruction generates a cryptographic trace that is
compressed into a STARK proof, then further compressed into a Groth16 SNARK
(~260 bytes, verifiable on-chain for ~300k gas).

### Data Flow

```
ProofInput (host → guest):
├── curve: CurveParams        # Oath-N curve parameters
├── window_size: usize        # Windowed scalar mul parameter
└── test_cases: Vec<TestCase>  # Random test inputs
    ├── a: u64                 # Scalar for generator G
    ├── b: u64                 # Scalar for target Q
    ├── target_q: AffinePoint  # Q = [k]G
    └── expected: AffinePoint  # Classical [a]G + [b]Q

ProofOutput (guest → verifier, public values):
├── qubit_count: usize         # Peak logical qubits
├── toffoli_count: usize       # Total Toffoli gates
├── cnot_count: usize          # Total CNOT gates
├── depth: usize               # Circuit depth
├── num_test_cases: usize      # Tests verified
├── field_bits: usize          # Oath-N tier identifier
├── window_size: usize         # Window parameter used
└── circuit_hash: [u8; 32]     # SHA-256 of CircuitSummary
```

### Kickmix Concept

The term "kickmix" refers to a custom quantum circuit format designed for
verifiability. Key properties:

- Circuits use only reversible logic gates (NOT, CNOT, Toffoli)
- No loops, subroutines, or branching — unambiguous circuit analysis
- Classically simulable on individual basis states
- SHA-256 hash binds the circuit to the proof
- Test inputs derived via Fiat-Shamir heuristic prevent selective testing

The Oathbreaker circuit follows this pattern: it builds a fully specified
reversible circuit from these gates, executes it classically on test inputs,
and proves the execution inside the SP1 zkVM.

## Proof Pipeline

### Step-by-step walkthrough

```
1. CURVE PARAMETERS
   └─> Load Oath-N params from sage/oath_all_params.json
       ├─ Field prime p (word-sized; Goldilocks 2^64 - 2^32 + 1 at Oath-64)
       ├─ Curve coefficients a, b
       ├─ Generator point G
       └─ Group order

2. TEST CASE GENERATION
   └─> For each of N test cases:
       ├─ Random scalars a, b, k
       ├─ Compute Q = [k]G (the "target public key")
       └─ Compute expected = [a]G + [b]Q (classical reference)

3. CIRCUIT CONSTRUCTION (inside zkVM)
   └─> build_group_action_circuit_jacobian(curve, window_size)
       ├─ Allocate exponent registers (a, b) + Jacobian accumulator (X, Y, Z)
       ├─ Build windowed scalar mul for [a]G and [b]Q
       ├─ Final Binary GCD inversion: Z → Z⁻¹
       ├─ Affine recovery: x = X·Z⁻², y = Y·Z⁻³
       └─ Record all resource counts

4. CIRCUIT IDENTIFICATION
   └─> SHA-256(serde_json(CircuitSummary)) = circuit_hash
       ├─ Binds proof to specific circuit structure
       └─ Prevents substituting a different circuit post-hoc

5. TEST CASE VERIFICATION (inside zkVM)
   └─> For each test case (a, b, Q):
       ├─ circuit.execute_classical(a, b, Q) = result
       ├─ assert result == expected
       └─ Proven correct by SP1 execution trace

6. PUBLIC VALUE COMMITMENT
   └─> ProofOutput committed as public values:
       ├─ Resource counts (qubits, Toffoli, CNOT, depth)
       ├─ Number of tests verified
       └─ Circuit hash

7. PROOF GENERATION
   └─> SP1 compresses execution trace:
       ├─ STARK proof (per-shard, then recursive combination)
       └─> Groth16 SNARK (BN254 curve, ~260 bytes)

8. VERIFICATION
   └─> Anyone with the verification key can confirm:
       ├─ Circuit has claimed resource counts
       ├─ Circuit correctly computes [a]G + [b]Q on N inputs
       └─ Circuit matches the committed hash
```

## Running Proofs

### Prerequisites

**Classical mode** (no prerequisites beyond Rust):
```bash
cd oathbreaker
cargo build --workspace
```

**SP1 proof generation** (requires SP1 toolchain):
```bash
# Install SP1 toolchain
curl -L https://sp1.succinct.xyz | bash
sp1up

# Verify installation
cargo prove --version
```

### Classical Mode (Default)

Builds the circuit and verifies test cases without generating a ZK proof.
Useful for development, testing, and CI.

```bash
# Quick test with Oath-8 (seconds)
cargo run --release -p sp1-host -- --tier oath-8 --num-cases 10

# Larger tier (minutes)
cargo run --release -p sp1-host -- --tier oath-32 --num-cases 5

# Cross-verify with Pollard's rho (small tiers only)
cargo run --release -p sp1-host -- --tier oath-8 --num-cases 10 --cross-verify

# Guest self-test
cargo run --release -p sp1-program
```

### SP1 Execute Mode

Runs the guest program inside the SP1 zkVM without generating a proof.
Validates serialization and guest logic quickly.

```bash
cargo run --release -p sp1-host --features sp1 -- \
    --tier oath-8 --num-cases 10 --mode execute
```

### SP1 Prove Mode

Generates a full Groth16 SNARK proof. This is slow (minutes to hours
depending on the tier and hardware).

```bash
# Generate proof for Oath-8 (fastest)
cargo run --release -p sp1-host --features sp1 -- \
    --tier oath-8 --num-cases 10 --mode prove

# Specify output directory
cargo run --release -p sp1-host --features sp1 -- \
    --tier oath-8 --num-cases 10 --mode prove --output-dir ./proofs
```

## Proof Artifacts

After proof generation, the `proofs/` directory contains:

| File | Contents |
|------|----------|
| `proof.bin` | Groth16 SNARK proof (~260 bytes) |
| `vk.bin` | Verification key for independent verification |
| `circuit_summary.json` | Public values: resource counts + circuit hash |

### Independent Verification

```bash
# Using the SP1 host
cargo run --release -p sp1-host --features sp1 -- verify proofs/proof.bin

# Using the SP1 CLI directly
sp1 verify --proof proofs/proof.bin --vk proofs/vk.bin
```

### On-chain Verification

The Groth16 proof is verifiable on Ethereum for approximately 300k gas
using the SP1 Solidity verifier contract. The public values (`ProofOutput`)
are ABI-encoded and verified alongside the proof.

## Tier Selection Guide

| Tier | Use Case | Time (Classical) | Time (SP1 Prove) |
|------|----------|-------------------|-------------------|
| **Oath-8** | Development, CI, quick tests | ~1 second | ~5 minutes |
| **Oath-16** | Integration testing | ~5 seconds | ~30 minutes |
| **Oath-32** | Production proofs, benchmarks | ~30 seconds | ~2 hours |
| **Oath-64** | Not recommended for ZKP | ~minutes | Impractical |

Oath-8 is recommended for development and CI. Oath-32 is the largest
tier practical for full proof generation.

## Relationship to Google's zkp_ecc

The Oathbreaker ZKP follows the same architecture as Google Quantum AI's
[zkp_ecc](https://github.com/tanujkhattar/zkp_ecc) project:

| Component | Google (zkp_ecc) | Oathbreaker |
|-----------|-----------------|-------------|
| Target curve | secp256k1 (256-bit) | Oath family (8-64 bit) |
| Circuit format | Kickmix (custom) | Reversible gates (NOT/CNOT/Toffoli) |
| Circuit simulator | Rust (lib/) | Rust (reversible-arithmetic/) |
| Proof system | SP1 zkVM → Groth16 | SP1 zkVM → Groth16 |
| Test generation | Fiat-Shamir heuristic | Random sampling |
| Test count | 9,024 | Configurable (default: 10) |
| Verification | SHA-256 circuit hash | SHA-256 CircuitSummary hash |
| Open source | Circuit withheld | Fully open |

The key difference: Google's circuit is proprietary (only the hash is public),
while Oathbreaker's circuit is fully open and auditable.
