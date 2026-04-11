# Sage Scripts for Curve Generation

These SageMath scripts generate and validate the **Oath-64** elliptic curve parameters for the Oathbreaker project.

## Prerequisites

Install SageMath: https://www.sagemath.org/download.html

## Usage

```bash
# Step 1: Generate curve parameters
sage generate_curve.sage

# Step 2: Verify order via independent computation
sage verify_order.sage

# Step 3: Run comprehensive validation
sage validate_params.sage
```

## Output

`curve_params.json` — contains:
- `a`, `b`: Weierstrass coefficients
- `p`: Field prime (2^64 - 2^32 + 1)
- `order`: Group order (#E(GF(p)))
- `generator_x`, `generator_y`: Generator point coordinates
- `embedding_degree`: For MOV resistance check

## Requirements for the curve

1. **Prime order**: #E(GF(p)) must be prime (no cofactor complications)
2. **Non-anomalous**: order != p (prevents Smart's attack)
3. **Embedding degree > 4**: Resists MOV/Weil pairing transfer
4. **Generator of full order**: [n]G = O for the group order n
