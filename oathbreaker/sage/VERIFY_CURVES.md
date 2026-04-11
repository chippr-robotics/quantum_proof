# Verifying Oath Curve Parameters with SageMath

This document explains how to independently verify that every generated Oath-N
curve satisfies the Oathbreaker security properties. All checks use
[SageMath](https://www.sagemath.org/) so that any human reviewer can reproduce
them with a single trusted tool.

## Prerequisites

Install SageMath (>= 9.0): https://www.sagemath.org/download.html

Verify it works:

```bash
sage --version
```

## Quick Start

A single script verifies all four tiers at once:

```bash
cd oathbreaker/sage
sage verify_all_oath_curves.sage
```

Or verify one tier at a time (the original Oath-64 scripts still work):

```bash
sage verify_order.sage        # Oath-64 order verification
sage validate_params.sage     # Oath-64 comprehensive validation
```

---

## What Gets Verified

Each Oath-N curve must pass **all** of the following checks. A failure on any
check means the curve is unsuitable for the Oathbreaker benchmark.

### 1. Field prime is prime

The field characteristic `p` must be prime. This is foundational — if `p` is
composite, `GF(p)` is not a field and elliptic curve arithmetic is undefined.

```python
sage: p = 251
sage: is_prime(p)
True
```

Expected primes per tier:

| Tier | p | Hex | Source |
|------|---|-----|--------|
| Oath-8 | 251 | 0xfb | Largest 8-bit prime |
| Oath-16 | 65521 | 0xfff1 | Largest 16-bit prime |
| Oath-32 | 4294967291 | 0xfffffffb | 2^32 - 5 |
| Oath-64 | 18446744069414584321 | 0xffffffff00000001 | 2^64 - 2^32 + 1 (Goldilocks) |

### 2. Curve is non-singular (discriminant != 0)

A Weierstrass curve `E: y^2 = x^3 + ax + b` is non-singular if and only if
its discriminant `Delta = -16(4a^3 + 27b^2)` is non-zero in GF(p). A zero
discriminant means the curve has a cusp or node and is not an elliptic curve.

```python
sage: F = GF(p)
sage: a, b = F(1), F(4)
sage: disc = -16 * (4*a^3 + 27*b^2)
sage: disc != 0
True
```

SageMath will also refuse to construct a singular curve:

```python
sage: E = EllipticCurve(F, [a, b])   # raises ArithmeticError if singular
```

### 3. Group order is independently computed via SEA

This is the most important check. SageMath uses the Schoof-Elkies-Atkin (SEA)
algorithm to compute `#E(GF(p))` from scratch, independent of whatever method
generated the parameters. The claimed order must match exactly.

```python
sage: E = EllipticCurve(F, [a, b])
sage: n_computed = E.order()
sage: n_claimed = 271
sage: n_computed == n_claimed
True
```

**Why this matters:** The generation script used baby-step giant-step for
larger fields. SEA is a completely independent algorithm. Agreement between
the two provides strong evidence the order is correct.

### 4. Group order is prime

A prime group order means there is no cofactor — every non-identity point is a
generator. This simplifies the Shor circuit (no cofactor clearing needed) and
eliminates small-subgroup attacks.

```python
sage: is_prime(n_computed)
True
```

### 5. Non-anomalous: order != p

An anomalous curve has `#E(GF(p)) = p`. Smart's attack solves ECDLP on
anomalous curves in linear time via a p-adic lift, making them cryptographically
useless. For the Oathbreaker benchmark, we need ECDLP to be hard enough that
only quantum methods are practical at each tier's scale.

```python
sage: n_computed != p
True
```

### 6. Embedding degree > 4

The embedding degree `k` is the multiplicative order of `p` modulo `n` — the
smallest positive integer such that `p^k ≡ 1 (mod n)`. If `k` is small,
the MOV/Weil pairing transfers ECDLP to DLP in `GF(p^k)`, which can be easier.
We require `k > 4` to prevent this.

```python
sage: k = Mod(p, n_computed).multiplicative_order()
sage: k > 4
True
sage: k
270
```

In practice, for random prime-order curves, `k` is enormous (close to `n`).
A small `k` would be a red flag.

### 7. Generator is on the curve

The claimed generator point `G = (gx, gy)` must satisfy the curve equation:
`gy^2 = gx^3 + a*gx + b (mod p)`.

```python
sage: gx, gy = F(0), F(2)
sage: G = E(gx, gy)          # raises TypeError if not on curve
sage: G in E
True
```

### 8. Generator has full order

Since the group order `n` is prime, every non-identity point has order `n`.
Verify that `[n]G = O` (the point at infinity):

```python
sage: n_computed * G == E(0)
True
```

Also verify `G` is not the identity:

```python
sage: G != E(0)
True
```

### 9. Generator is not a small-order point

As an extra safety check, verify that no small multiple of G is the identity.
For a prime-order group this is guaranteed, but it catches implementation bugs:

```python
sage: all(k * G != E(0) for k in [2, 3, 5, 7, 11, 13])
True
```

### 10. Hasse bound satisfied

The Hasse-Weil theorem guarantees `|#E(GF(p)) - (p + 1)| <= 2*sqrt(p)`. This
is a mathematical invariant — if violated, something is deeply wrong.

```python
sage: t = p + 1 - n_computed          # trace of Frobenius
sage: abs(t) <= 2 * isqrt(p)
True
```

### 11. Twist security (informational)

The quadratic twist `E'` of `E` has order `n' = 2p + 2 - n`. For full
protection in protocols that might accidentally operate on the twist, the
largest prime factor of `n'` should be large. This is informational for the
benchmark (not a hard requirement), but good practice.

```python
sage: n_twist = 2*p + 2 - n_computed
sage: factor(n_twist)
```

### 12. CM discriminant (informational)

The complex multiplication discriminant `D = t^2 - 4p` classifies the curve's
endomorphism ring. This is recorded for reference; no specific value is
required.

```python
sage: D = (p + 1 - n_computed)^2 - 4*p
sage: D
```

---

## Full Walkthrough: Verifying Oath-8

Below is a complete SageMath session verifying the Oath-8 curve. Copy-paste
this into a `sage` interactive session.

```python
# ── Load parameters ──
p = 251
a, b = 1, 4
n_claimed = 271
gx, gy = 0, 2

# ── Check 1: Field prime ──
assert is_prime(p), "FAIL: p is not prime"
print(f"[PASS] p = {p} is prime")

# ── Check 2: Construct curve (non-singular) ──
F = GF(p)
E = EllipticCurve(F, [a, b])
print(f"[PASS] Curve is non-singular")

# ── Check 3: Order via SEA ──
n = E.order()
assert n == n_claimed, f"FAIL: order mismatch (computed {n}, claimed {n_claimed})"
print(f"[PASS] Order verified: {n}")

# ── Check 4: Prime order ──
assert is_prime(n), "FAIL: order is not prime"
print(f"[PASS] Order is prime")

# ── Check 5: Non-anomalous ──
assert n != p, "FAIL: curve is anomalous (n == p)"
print(f"[PASS] Non-anomalous: n = {n} != p = {p}")

# ── Check 6: Embedding degree ──
k = Mod(p, n).multiplicative_order()
assert k > 4, f"FAIL: embedding degree k = {k} <= 4"
print(f"[PASS] Embedding degree k = {k} > 4")

# ── Check 7-9: Generator ──
G = E(gx, gy)
assert G != E(0), "FAIL: generator is the identity"
assert n * G == E(0), "FAIL: [n]G != O"
assert all(j * G != E(0) for j in [2,3,5,7,11,13]), "FAIL: small-order subpoint"
print(f"[PASS] Generator G = ({gx}, {gy}) has full order {n}")

# ── Check 10: Hasse bound ──
t = int(p) + 1 - int(n)
assert abs(t) <= 2 * isqrt(int(p)), "FAIL: Hasse bound violated"
print(f"[PASS] Hasse bound: |t| = {abs(t)} <= {2 * isqrt(int(p))}")

# ── Info: Twist ──
n_twist = 2*int(p) + 2 - int(n)
print(f"[INFO] Twist order: {n_twist} = {factor(n_twist)}")

# ── Info: CM discriminant ──
D = t^2 - 4*int(p)
print(f"[INFO] CM discriminant D = {D}")

print("\nAll checks passed for Oath-8.")
```

---

## Per-Tier Verification Parameters

Use the values below for each tier. The procedure is identical — only the
constants change.

### Oath-8

```python
p = 251
a, b = 1, 4
n_claimed = 271
gx, gy = 0, 2
```

### Oath-16

```python
p = 65521
a, b = 1, 35
n_claimed = 65761
gx, gy = 0, 22627
```

### Oath-32

```python
p = 4294967291          # 2^32 - 5
a, b = 1, 13
n_claimed = 4295040499
gx, gy = 0, 929806792
```

### Oath-64

```python
p = 2^64 - 2^32 + 1    # 18446744069414584321 (Goldilocks prime)
a, b = 1, 38
n_claimed = 18446744077729562113
gx, gy = 1, 4519977769586765578
```

---

## Automated Verification Script

The script `verify_all_oath_curves.sage` runs all checks for all tiers in one
invocation:

```bash
sage verify_all_oath_curves.sage
```

It loads each `oath{N}_params.json` file, runs the full check suite, and prints
a PASS/FAIL summary. See the script source for details.

---

## Interpreting Failures

| Check | Failure means |
|-------|--------------|
| Prime not prime | Corrupted parameter file |
| Singular curve | Bad (a, b) coefficients |
| Order mismatch | Generation bug — the order was computed incorrectly |
| Order not prime | Curve has a cofactor; unsuitable for Oathbreaker |
| Anomalous | Smart's attack applies; ECDLP is trivial |
| Embedding degree <= 4 | MOV attack transfers ECDLP to easier DLP |
| Generator not on curve | Corrupted generator coordinates |
| [n]G != O | Order or generator is wrong |
| Hasse violation | Mathematically impossible — indicates a severe bug |

---

## Trust Model

The verification process is designed so that a reviewer needs to trust only
SageMath (and its underlying PARI/GP library for `E.order()`). The generation
script (`generate_oath_curves.py`) is deliberately **not** trusted — its output
is verified from scratch by an independent algorithm (SEA) in a separate tool.
