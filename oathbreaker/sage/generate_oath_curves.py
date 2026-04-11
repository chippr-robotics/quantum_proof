#!/usr/bin/env python3
"""
generate_oath_curves.py — Generate Oath-N curve parameters for all benchmark tiers.

Generates safe elliptic curves E: y^2 = x^3 + ax + b over GF(p) for each
Oath-N tier (Oath-8, Oath-16, Oath-32, Oath-64).

Each curve satisfies:
  - Prime group order (no cofactor)
  - Non-anomalous (order != p)
  - Embedding degree > 4  (resists MOV/Weil pairing transfer)
  - Generator of full order verified

Pure Python + sympy implementation (no SageMath required).

For small fields (8, 16-bit) uses brute-force point enumeration.
For larger fields (32, 64-bit) uses baby-step giant-step on the Hasse interval.

Output: oath{8,16,32,64}_params.json and oath_all_params.json
"""

import json
import math
import os
import sys
import time
from sympy import isprime, factorint


# ── Tier definitions ─────────────────────────────────────────────

TIERS = [
    {"name": "Oath-8",  "bits": 8,  "p": 251},
    {"name": "Oath-16", "bits": 16, "p": 65521},
    {"name": "Oath-32", "bits": 32, "p": 4294967291},
    {"name": "Oath-64", "bits": 64, "p": (1 << 64) - (1 << 32) + 1},
]


# ── Elliptic curve arithmetic (affine, short Weierstrass) ───────

INF = None  # Point at infinity


def ec_add(P, Q, a, p):
    """Add two points on E: y^2 = x^3 + ax + b over GF(p)."""
    if P is INF:
        return Q
    if Q is INF:
        return P
    x1, y1 = P
    x2, y2 = Q
    if x1 == x2:
        if y1 != y2:
            return INF
        if y1 == 0:
            return INF
        lam = (3 * x1 * x1 + a) * pow(2 * y1, -1, p) % p
    else:
        lam = (y2 - y1) * pow(x2 - x1, -1, p) % p
    x3 = (lam * lam - x1 - x2) % p
    y3 = (lam * (x1 - x3) - y1) % p
    return (x3, y3)


def ec_neg(P, p):
    """Negate a point on E."""
    if P is INF:
        return INF
    return (P[0], (-P[1]) % p)


def ec_mul(k, P, a, p):
    """Scalar multiplication k*P via double-and-add."""
    if k == 0 or P is INF:
        return INF
    if k < 0:
        P = ec_neg(P, p)
        k = -k
    R = INF
    Q = P
    while k:
        if k & 1:
            R = ec_add(R, Q, a, p)
        Q = ec_add(Q, Q, a, p)
        k >>= 1
    return R


# ── Modular square root (Tonelli-Shanks) ────────────────────────

def sqrt_mod(n, p):
    """Compute sqrt(n) mod p. Returns None if n is not a QR."""
    n = n % p
    if n == 0:
        return 0
    if pow(n, (p - 1) // 2, p) != 1:
        return None
    q, s = p - 1, 0
    while q % 2 == 0:
        q //= 2
        s += 1
    if s == 1:
        return pow(n, (p + 1) // 4, p)
    z = 2
    while pow(z, (p - 1) // 2, p) != p - 1:
        z += 1
    m, c, t, r = s, pow(z, q, p), pow(n, q, p), pow(n, (q + 1) // 2, p)
    while True:
        if t == 1:
            return r
        i, tmp = 1, (t * t) % p
        while tmp != 1:
            tmp = (tmp * tmp) % p
            i += 1
        b = c
        for _ in range(m - i - 1):
            b = (b * b) % p
        m, c = i, (b * b) % p
        t = (t * c) % p
        r = (r * b) % p


# ── Point finding ───────────────────────────────────────────────

def find_point(a, b, p, skip_zero_y=False):
    """Find a point on E with the smallest x-coordinate (deterministic)."""
    limit = min(p, 100000)
    for x in range(limit):
        rhs = (x * x * x + a * x + b) % p
        if rhs == 0:
            if skip_zero_y:
                continue
            return (x, 0)
        y = sqrt_mod(rhs, p)
        if y is not None:
            return (x, min(y, p - y))
    return None


# ── Point counting ──────────────────────────────────────────────

def count_points_brute(a, b, p):
    """Count #E(GF(p)) by enumerating all x in GF(p). O(p)."""
    count = 1  # Point at infinity
    for x in range(p):
        rhs = (x * x * x + a * x + b) % p
        if rhs == 0:
            count += 1
        elif pow(rhs, (p - 1) // 2, p) == 1:
            count += 2
    return count


def bsgs_curve_order(a, b, p):
    """
    Compute #E(GF(p)) via baby-step giant-step on the Hasse interval.

    By Hasse's theorem, #E in [p+1-2*sqrt(p), p+1+2*sqrt(p)].
    We find n in that interval such that n*P = O for a random point P,
    then verify. For prime-order curves, any non-identity point is a
    generator, so this recovers #E exactly.
    """
    P = find_point(a, b, p, skip_zero_y=True)
    if P is None:
        return None

    sqrt_p = math.isqrt(p)
    L = p + 1 - 2 * sqrt_p
    H = p + 1 + 2 * sqrt_p
    W = H - L
    m = math.isqrt(W) + 1

    # We want n in [L, H] with n*P = O.
    # Let Q = L*P, target = -Q. Then r*P = target where n = L + r.
    Q = ec_mul(L, P, a, p)
    target = ec_neg(Q, p)

    # Baby steps: j*P for j = 0 .. m-1
    baby = {}
    jP = INF
    for j in range(m):
        if jP is INF:
            key = 'INF'
            val = (j, None)
        else:
            key = jP[0]
            val = (j, jP[1])
        if key not in baby:
            baby[key] = val
        jP = ec_add(jP, P, a, p)

    # Giant steps: target - i*m*P for i = 0, 1, ...
    mP = ec_mul(m, P, a, p)
    neg_mP = ec_neg(mP, p)
    R = target

    for i in range(m + 2):
        if R is INF:
            key = 'INF'
        else:
            key = R[0]

        if key in baby:
            j, y_j = baby[key]
            if R is INF or y_j is None or R[1] == y_j:
                n = L + i * m + j
            else:
                n = L + i * m - j
            if L <= n <= H:
                # Verify: n * P = O
                if ec_mul(n, P, a, p) is INF:
                    return n

        R = ec_add(R, neg_mP, a, p)

    return None


# ── Embedding degree ────────────────────────────────────────────

def compute_embedding_degree(p, n):
    """
    Multiplicative order of p mod n (n must be prime).
    The embedding degree k must be > 4 to resist MOV/Weil pairing attacks.
    """
    # Quick check for small k
    val = 1
    for k in range(1, 5):
        val = val * p % n
        if val == 1:
            return k

    # Full computation via factoring phi(n) = n-1
    phi = n - 1
    factors = factorint(phi)
    order = phi
    for q in factors:
        while order % q == 0 and pow(p, order // q, n) == 1:
            order //= q
    return order


# ── Curve checking ──────────────────────────────────────────────

def check_curve(a_int, b_int, p, use_brute):
    """
    Check if E: y^2 = x^3 + a*x + b over GF(p) meets all Oath criteria.
    Returns parameter dict on success, None on failure.
    """
    a = a_int % p
    b = b_int % p

    # Non-singular: discriminant != 0
    disc = (-16 * (4 * a**3 + 27 * b**2)) % p
    if disc == 0:
        return None

    # Count points
    if use_brute:
        n = count_points_brute(a, b, p)
    else:
        n = bsgs_curve_order(a, b, p)
    if n is None:
        return None

    # Prime order (no cofactor)
    if not isprime(n):
        return None

    # Non-anomalous: n != p (prevents Smart's attack)
    if n == p:
        return None

    # Embedding degree > 4 (resists MOV/Weil pairing transfer)
    k = compute_embedding_degree(p, n)
    if k <= 4:
        return None

    # Find generator (deterministic: smallest x, smaller y)
    G = find_point(a, b, p)
    if G is None:
        return None

    # Verify: n * G = O (generator has full order)
    if ec_mul(n, G, a, p) is not INF:
        return None

    return {
        "a": a_int,
        "b": b_int,
        "p": int(p),
        "order": int(n),
        "generator_x": int(G[0]),
        "generator_y": int(G[1]),
        "embedding_degree": int(k),
        "discriminant": int(disc),
    }


# ── Main ────────────────────────────────────────────────────────

def main():
    out_dir = os.path.dirname(os.path.abspath(__file__))

    print("=" * 60)
    print("Oath Curve Generator")
    print("Generating parameters for all Oath-N benchmark tiers")
    print("=" * 60)
    print()

    # Verify field primes
    for tier in TIERS:
        p = tier["p"]
        assert isprime(p), f"FATAL: p = {p} is not prime"
    print("All field primes verified.\n")

    all_results = {}

    for tier in TIERS:
        name = tier["name"]
        bits = tier["bits"]
        p = tier["p"]
        use_brute = bits <= 16

        print(f"--- {name} ---")
        print(f"  Field: GF({p})")
        print(f"  p = {hex(p)} ({p.bit_length()}-bit)")
        print(f"  Method: {'brute-force enumeration' if use_brute else 'baby-step giant-step'}")
        sys.stdout.flush()

        t0 = time.time()
        found = False
        curves_checked = 0

        for a_int in range(1, 200):
            if found:
                break
            for b_int in range(1, 200):
                curves_checked += 1
                result = check_curve(a_int, b_int, p, use_brute)
                if result is not None:
                    elapsed = time.time() - t0
                    result["tier"] = name
                    result["field_bits"] = bits

                    print(f"  Found suitable curve (checked {curves_checked} candidates)")
                    print(f"  E: y^2 = x^3 + {a_int}x + {b_int}")
                    print(f"  Order n = {result['order']} (prime)")
                    print(f"  Non-anomalous: n != p: True")
                    print(f"  Embedding degree k = {result['embedding_degree']}")
                    print(f"  Generator G = ({result['generator_x']}, {result['generator_y']})")
                    print(f"  Time: {elapsed:.2f}s")

                    fname = f"oath{bits}_params.json"
                    fpath = os.path.join(out_dir, fname)
                    with open(fpath, 'w') as f:
                        json.dump(result, f, indent=2)
                    print(f"  -> {fname}")
                    print()

                    all_results[name] = result
                    found = True
                    break

        if not found:
            print(f"  ERROR: no suitable curve found in search range")
            print()
            sys.exit(1)

    # Combined output
    combined_path = os.path.join(out_dir, "oath_all_params.json")
    with open(combined_path, 'w') as f:
        json.dump(all_results, f, indent=2)
    print(f"Combined parameters -> oath_all_params.json")

    print()
    print("=" * 60)
    print("All Oath curves generated successfully.")
    print("=" * 60)


if __name__ == "__main__":
    main()
