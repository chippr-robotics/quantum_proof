# generate_curve.sage
# Generate the Oath-64 curve: a safe elliptic curve E: y^2 = x^3 + ax + b
# over GF(p) where p = 2^64 - 2^32 + 1 (the Goldilocks prime).
#
# Requirements:
# - Group order n = #E(GF(p)) must be prime (no cofactor)
# - Non-anomalous: n != p
# - Embedding degree k > 4
# - Generator G of order n identified
#
# Usage: sage generate_curve.sage
#
# Output: oath64_params.json

import json
import sys

p = 2^64 - 2^32 + 1
F = GF(p)

print(f"Goldilocks prime p = {p}")
print(f"p = 2^64 - 2^32 + 1 = {hex(p)}")
print(f"Searching for suitable curve E/GF(p)...\n")

def check_curve(a, b):
    """Check if E: y^2 = x^3 + ax + b over GF(p) is suitable."""
    # Check discriminant
    disc = -16 * (4 * a^3 + 27 * b^2)
    if disc % p == 0:
        return None

    E = EllipticCurve(F, [a, b])
    n = E.order()

    # Must have prime order
    if not is_prime(n):
        return None

    # Non-anomalous: n != p
    if n == p:
        return None

    # Embedding degree k = multiplicative_order(p, n)
    # Must be > 4 to resist MOV/Weil pairing transfer
    k = Mod(p, n).multiplicative_order()
    if k <= 4:
        return None

    # Find a generator
    G = E.random_point()
    while G == E(0):
        G = E.random_point()

    # Verify G has order n
    assert n * G == E(0), "Generator does not have full order"

    return {
        'a': int(a),
        'b': int(b),
        'p': int(p),
        'order': int(n),
        'generator_x': int(G[0]),
        'generator_y': int(G[1]),
        'embedding_degree': int(k),
        'discriminant': int(disc % p),
    }


# Search over small coefficients
found = False
for a_int in range(1, 100):
    for b_int in range(1, 100):
        a = F(a_int)
        b = F(b_int)
        result = check_curve(a, b)
        if result is not None:
            print(f"Found suitable curve!")
            print(f"  E: y^2 = x^3 + {a_int}x + {b_int}")
            print(f"  over GF({p})")
            print(f"  Order n = {result['order']}")
            print(f"  n is prime: {is_prime(result['order'])}")
            print(f"  Non-anomalous: n != p: {result['order'] != int(p)}")
            print(f"  Embedding degree k = {result['embedding_degree']}")
            print(f"  Generator G = ({result['generator_x']}, {result['generator_y']})")
            print()

            # Write output
            with open('oath64_params.json', 'w') as f:
                json.dump(result, f, indent=2)
            print(f"Parameters written to oath64_params.json")
            found = True
            break
    if found:
        break

if not found:
    print("ERROR: No suitable curve found in search range.")
    print("Try expanding the search range for a and b.")
    sys.exit(1)
