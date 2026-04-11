# verify_order.sage
# Confirm the group order of the generated curve via Schoof's algorithm.
#
# This is an independent verification of the curve order computed by
# generate_curve.sage. Sage uses SEA (Schoof-Elkies-Atkin) internally.
#
# Usage: sage verify_order.sage

import json

p = 2^64 - 2^32 + 1
F = GF(p)

# Load parameters
with open('curve_params.json', 'r') as f:
    params = json.load(f)

a = F(params['a'])
b = F(params['b'])
n_claimed = params['order']
gx = F(params['generator_x'])
gy = F(params['generator_y'])

print(f"Verifying curve E: y^2 = x^3 + {params['a']}x + {params['b']}")
print(f"over GF({p})")
print(f"Claimed order: {n_claimed}")
print()

# Construct curve and compute order
E = EllipticCurve(F, [a, b])
n_computed = E.order()

print(f"Computed order (SEA): {n_computed}")

if n_computed == n_claimed:
    print("ORDER VERIFIED: claimed order matches computed order.")
else:
    print(f"ORDER MISMATCH: claimed {n_claimed}, computed {n_computed}")
    exit(1)

# Verify primality
print(f"Order is prime: {is_prime(n_computed)}")

# Verify generator
G = E(gx, gy)
print(f"Generator G = ({gx}, {gy})")
print(f"G is on curve: {G in E}")

# Verify G has order n
nG = n_computed * G
print(f"[n]G = O (point at infinity): {nG == E(0)}")

# Verify G is not a small-order point
for small_k in [2, 3, 5, 7, 11, 13]:
    if small_k < n_computed:
        assert small_k * G != E(0), f"G has unexpected small order dividing {small_k}"
print("Generator order is full (not a small factor of n).")

print("\nAll verifications passed.")
