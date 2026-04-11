# validate_params.sage
# Comprehensive validation of curve parameters:
# - Non-anomalous (n != p)
# - Embedding degree > 4
# - Twist security
# - CM discriminant
#
# Usage: sage validate_params.sage

import json

p = 2^64 - 2^32 + 1
F = GF(p)

# Load parameters
with open('curve_params.json', 'r') as f:
    params = json.load(f)

a = F(params['a'])
b = F(params['b'])
n = params['order']

E = EllipticCurve(F, [a, b])

print("=== Comprehensive Curve Parameter Validation ===\n")
print(f"Curve: E: y^2 = x^3 + {params['a']}x + {params['b']}")
print(f"Field: GF({p})")
print(f"Order: {n}\n")

all_pass = True

# 1. Order is prime
check_prime = is_prime(n)
print(f"[{'PASS' if check_prime else 'FAIL'}] Order is prime: {check_prime}")
all_pass &= check_prime

# 2. Non-anomalous: n != p (prevents Smart's attack)
check_anomalous = (n != int(p))
print(f"[{'PASS' if check_anomalous else 'FAIL'}] Non-anomalous (n != p): {check_anomalous}")
all_pass &= check_anomalous

# 3. Embedding degree > 4 (prevents MOV/Weil pairing transfer)
k = Mod(p, n).multiplicative_order()
check_embedding = (k > 4)
print(f"[{'PASS' if check_embedding else 'FAIL'}] Embedding degree k = {k} > 4: {check_embedding}")
all_pass &= check_embedding

# 4. Twist security
# The quadratic twist E' has order n' = 2p + 2 - n
n_twist = 2 * int(p) + 2 - n
# Factor n' and check largest prime factor
n_twist_factors = factor(n_twist)
largest_prime_factor = max(f[0] for f in n_twist_factors)
cofactor = n_twist // largest_prime_factor
check_twist = (largest_prime_factor.nbits() >= 48)  # Reasonable for 64-bit
print(f"[{'PASS' if check_twist else 'WARN'}] Twist order: {n_twist}")
print(f"    Twist factorization: {n_twist_factors}")
print(f"    Largest prime factor: {largest_prime_factor} ({largest_prime_factor.nbits()} bits)")
print(f"    Twist cofactor: {cofactor}")

# 5. Discriminant
disc = -16 * (4 * int(a)^3 + 27 * int(b)^2) % int(p)
check_disc = (disc != 0)
print(f"[{'PASS' if check_disc else 'FAIL'}] Discriminant non-zero: {check_disc}")
all_pass &= check_disc

# 6. Trace of Frobenius
t = int(p) + 1 - n
print(f"[INFO] Trace of Frobenius t = {t}")
print(f"[INFO] |t| = {abs(t)}")
print(f"[INFO] Hasse bound: |t| <= 2*sqrt(p) = {2 * isqrt(int(p))}")
check_hasse = (abs(t) <= 2 * isqrt(int(p)))
print(f"[{'PASS' if check_hasse else 'FAIL'}] Hasse bound satisfied: {check_hasse}")
all_pass &= check_hasse

# 7. CM discriminant
D = t^2 - 4 * int(p)
print(f"[INFO] CM discriminant D = {D}")

print(f"\n{'='*50}")
if all_pass:
    print("ALL CHECKS PASSED - curve is suitable for Shor circuit.")
else:
    print("SOME CHECKS FAILED - review parameters.")
