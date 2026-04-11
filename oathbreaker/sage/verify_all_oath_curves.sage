# verify_all_oath_curves.sage
#
# Comprehensive SageMath verification of all Oath-N curve parameters.
# Runs every security check for every tier (Oath-8, Oath-16, Oath-32, Oath-64).
#
# Each check uses SageMath's own algorithms (SEA for point counting, etc.)
# to independently verify the parameters produced by generate_oath_curves.py.
#
# Usage: sage verify_all_oath_curves.sage
#
# Requires: oath{8,16,32,64}_params.json in the same directory.

import json
import os
import sys

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__)) if '__file__' in dir() else '.'

TIER_FILES = [
    ("Oath-8",  "oath8_params.json"),
    ("Oath-16", "oath16_params.json"),
    ("Oath-32", "oath32_params.json"),
    ("Oath-64", "oath64_params.json"),
]

overall_pass = True
results = []

print("=" * 64)
print("  Oathbreaker Curve Verification Suite (SageMath)")
print("  Independent verification of all Oath-N curve parameters")
print("=" * 64)
print()

for tier_name, filename in TIER_FILES:
    filepath = os.path.join(SCRIPT_DIR, filename)
    print(f"{'─' * 64}")
    print(f"  {tier_name}  ({filename})")
    print(f"{'─' * 64}")

    # ── Load parameters ──────────────────────────────────────────

    if not os.path.exists(filepath):
        print(f"  [SKIP] File not found: {filename}")
        print()
        results.append((tier_name, "SKIP"))
        continue

    with open(filepath, 'r') as f:
        params = json.load(f)

    p_int = params['p']
    a_int = params['a']
    b_int = params['b']
    n_claimed = params['order']
    gx_int = params['generator_x']
    gy_int = params['generator_y']
    k_claimed = params['embedding_degree']

    print(f"  Curve:     E: y^2 = x^3 + {a_int}x + {b_int}")
    print(f"  Field:     GF({p_int})")
    print(f"  Claimed n: {n_claimed}")
    print(f"  Claimed G: ({gx_int}, {gy_int})")
    print(f"  Claimed k: {k_claimed}")
    print()

    tier_pass = True

    # ── Check 1: Field prime is prime ────────────────────────────

    check = is_prime(p_int)
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Field prime p is prime")

    # ── Check 2: Non-singular (discriminant != 0) ────────────────

    F = GF(p_int)
    a = F(a_int)
    b = F(b_int)
    disc = -16 * (4*a^3 + 27*b^2)
    check = (disc != 0)
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Non-singular: discriminant = {disc} != 0")

    # Construct curve (also fails if singular)
    E = EllipticCurve(F, [a, b])

    # ── Check 3: Order via SEA matches claimed order ─────────────

    n_computed = E.order()
    check = (int(n_computed) == n_claimed)
    tier_pass &= check
    if check:
        print(f"  [PASS] Order verified via SEA: {n_computed}")
    else:
        print(f"  [FAIL] Order mismatch: SEA computed {n_computed}, claimed {n_claimed}")

    # ── Check 4: Order is prime ──────────────────────────────────

    check = is_prime(n_computed)
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Order is prime")

    # ── Check 5: Non-anomalous ───────────────────────────────────

    check = (int(n_computed) != p_int)
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Non-anomalous: n != p")

    # ── Check 6: Embedding degree > 4 ───────────────────────────

    k_computed = Mod(p_int, int(n_computed)).multiplicative_order()
    check_k_threshold = (k_computed > 4)
    tier_pass &= check_k_threshold
    print(f"  [{'PASS' if check_k_threshold else 'FAIL'}] Embedding degree k = {k_computed} > 4")

    check_k_match = (int(k_computed) == k_claimed)
    if check_k_match:
        print(f"  [PASS] Embedding degree matches claimed value")
    else:
        print(f"  [WARN] Embedding degree mismatch: computed {k_computed}, claimed {k_claimed}")

    # ── Check 7: Generator is on the curve ───────────────────────

    gx = F(gx_int)
    gy = F(gy_int)
    try:
        G = E(gx, gy)
        check = True
    except TypeError:
        check = False
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Generator G = ({gx_int}, {gy_int}) is on curve")

    # ── Check 8: Generator is not the identity ───────────────────

    if check:
        check_nonzero = (G != E(0))
        tier_pass &= check_nonzero
        print(f"  [{'PASS' if check_nonzero else 'FAIL'}] Generator is not the point at infinity")

    # ── Check 9: [n]G = O ────────────────────────────────────────

    if check:
        nG = int(n_computed) * G
        check_order = (nG == E(0))
        tier_pass &= check_order
        print(f"  [{'PASS' if check_order else 'FAIL'}] [n]G = O (generator has full order)")

    # ── Check 10: No small-order subpoint ────────────────────────

    if check:
        small_primes = [2, 3, 5, 7, 11, 13, 17, 19, 23]
        small_order_ok = all(
            k_test * G != E(0)
            for k_test in small_primes
            if k_test < int(n_computed)
        )
        tier_pass &= small_order_ok
        print(f"  [{'PASS' if small_order_ok else 'FAIL'}] Generator passes small-order test")

    # ── Check 11: Hasse bound ────────────────────────────────────

    t = p_int + 1 - int(n_computed)
    hasse_bound = 2 * isqrt(p_int)
    check = (abs(t) <= hasse_bound)
    tier_pass &= check
    print(f"  [{'PASS' if check else 'FAIL'}] Hasse bound: |t| = {abs(t)} <= {hasse_bound}")

    # ── Info: Trace of Frobenius ─────────────────────────────────

    print(f"  [INFO] Trace of Frobenius t = {t}")

    # ── Info: Twist security ─────────────────────────────────────

    n_twist = 2 * p_int + 2 - int(n_computed)
    n_twist_factors = factor(n_twist)
    largest_pf = max(f[0] for f in n_twist_factors)
    print(f"  [INFO] Twist order: {n_twist}")
    print(f"         Factorization: {n_twist_factors}")
    print(f"         Largest prime factor: {largest_pf} ({largest_pf.nbits()} bits)")

    # ── Info: CM discriminant ────────────────────────────────────

    D = t^2 - 4 * p_int
    print(f"  [INFO] CM discriminant D = {D}")

    # ── Tier summary ─────────────────────────────────────────────

    status = "PASS" if tier_pass else "FAIL"
    overall_pass &= tier_pass
    results.append((tier_name, status))
    print()
    print(f"  >>> {tier_name}: {'ALL CHECKS PASSED' if tier_pass else 'SOME CHECKS FAILED'}")
    print()


# ── Final summary ────────────────────────────────────────────────

print("=" * 64)
print("  VERIFICATION SUMMARY")
print("=" * 64)
for name, status in results:
    symbol = "+" if status == "PASS" else ("-" if status == "SKIP" else "X")
    print(f"  [{symbol}] {name}: {status}")
print()
if overall_pass:
    print("  ALL TIERS PASSED — curves are suitable for the Oathbreaker benchmark.")
else:
    print("  SOME TIERS FAILED — review output above.")
print("=" * 64)

if not overall_pass:
    sys.exit(1)
