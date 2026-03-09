# Orbital AMM — Mathematical Reference

> Source: [Paradigm — Orbital](https://www.paradigm.xyz/2025/06/orbital)
>
> This document captures the key mathematical definitions, invariants, and formulas
> from the Paradigm Orbital paper for implementation reference.

---

## 1. Notation & Definitions

| Symbol | Definition |
|--------|-----------|
| `n` | Number of assets in the pool |
| `r` | Sphere radius parameter |
| `x_i` | Reserve of asset `i` (i = 1..n) |
| `x⃗` | Reserve vector `(x_1, x_2, ..., x_n)` |
| `r⃗` | Center vector `(r, r, ..., r)` |
| `v⃗` | Unit direction vector `(1,1,...,1)/√n` |
| `α` | Parallel projection of `x⃗` onto `v⃗` |
| `w⃗` | Orthogonal component: `x⃗ - αv⃗` |
| `k` | Tick boundary plane constant |
| `q` | Equal price point (balanced reserve per asset) |

---

## 2. Sphere Invariant (Core AMM)

The Orbital AMM operates on an n-dimensional sphere:

```
||r⃗ - x⃗||² = Σᵢ(r - xᵢ)² = r²
```

This invariant defines the trading surface. All valid reserve states lie on this sphere.

### Equivalent expanded form (O(1) computation):

```
n·r² - 2r·Σxᵢ + Σxᵢ² = r²
```

---

## 3. Marginal Price

The exchange rate between token `i` and token `j`:

```
price(i, j) = δxᵢ/δxⱼ = (r - xⱼ) / (r - xᵢ)
```

- At equal reserves: `price(i, j) = 1.0` for all pairs
- When `xᵢ < xⱼ` (token `i` is scarcer): `price(i, j) < 1.0`
  - You receive less of the scarce token per unit of the abundant token deposited

---

## 4. Equal Price Point

The balanced reserve state where all tokens trade at parity:

```
q = r · (1 - 1/√n)
```

Each asset holds `q` units of reserve. At this point, all marginal prices equal 1.0.

---

## 5. Polar Decomposition

Any reserve vector decomposes into parallel and orthogonal components:

```
x⃗ = α·v⃗ + w⃗    where v⃗ ⊥ w⃗
```

### Alpha (parallel projection)

```
α = x⃗ · v⃗ = Σxᵢ / √n
```

Alpha measures how far the aggregate reserves are from the center along the diagonal.
It changes monotonically during swaps → used for tick crossing detection.

### W norm squared (orthogonal component)

```
||w⃗||² = Σxᵢ² - (Σxᵢ)²/n
```

Measures deviation from the equal-price diagonal. Zero when all reserves are equal.

> **On the sphere surface**, this is equivalent to:
> `||w⃗||² = r² - (α - r√n)²`
>
> Our implementation uses the general form `Σxᵢ² - (Σxᵢ)²/n` which works
> both on and off the sphere surface.

---

## 6. Tick Structure

Orbital uses nested spherical caps (ticks) for concentrated liquidity.

### Tick boundary

```
x⃗ · v⃗ = k    (plane perpendicular to v⃗)
```

### Tick boundary range

```
k_min = r · (√n - 1)
k_max = r · (n - 1) / √n
```

### Reserve bounds within a tick

```
x_min = [k√n - √(k²n - n·((n-1)r - k√n)²)] / n
x_max = min(r, [k√n + √(k²n - n·((n-1)r - k√n)²)] / n)
```

### Orthogonal subspace radius (boundary tick)

```
s = √(r² - (k - r√n)²)
```

The boundary tick behaves as an `(n-1)`-dimensional sphere in the subspace orthogonal to `v⃗`.

---

## 7. Torus Invariant (Global Trade)

For constant-time trade computation across combined ticks:

```
r_int² = (x⃗_total · v⃗ - k_bound - r_int·√n)²
       + (||x⃗_total - (x⃗_total · v⃗)·v⃗|| - √(r_bound² - (k_bound - r_bound·√n)²))²
```

This combines interior and boundary tick liquidity into a torus (donut) shape in a higher-dimensional space, enabling O(1) swap computation.

---

## 8. Tick Consolidation

### Interior ticks (parallel reserves)

Two ticks with radii `r_a` and `r_b` consolidate as:

```
x⃗_a = (r_a / r_b) · x⃗_b
r_combined = r_a + r_b
```

### Boundary ticks (orthogonal trades)

```
Δa⃗ · v⃗ = 0,   Δb⃗ · v⃗ = 0
s_combined = s_a + s_b
```

---

## 9. Capital Efficiency

```
c_efficiency(p) = x_base / (x_base - x_min(k_depeg(p)))
```

Where:
- `p` = maximum depeg price the LP covers
- `x_base = r(1 - 1/√n)` = base reserve at equal price point
- `k_depeg(p)` = tick boundary corresponding to depeg price `p`

---

## 10. Implementation Mapping

| Paper Concept | Our Code | Location |
|--------------|----------|----------|
| Sphere invariant `\|\|r⃗-x⃗\|\|²=r²` | `Sphere::verify_invariant()` | `math/sphere.rs` |
| `Σ(r-xᵢ)²` (O(n)) | `Sphere::distance_squared()` | `math/sphere.rs` |
| `n·r²-2r·Σxᵢ+Σxᵢ²` (O(1)) | `ReserveState::distance_squared_from_center()` | `math/reserve_state.rs` |
| `price(i,j) = (r-xⱼ)/(r-xᵢ)` | `Sphere::price()`, `ReserveState::price()` | both files |
| `q = r(1-1/√n)` | `Sphere::equal_price_point()` | `math/sphere.rs` |
| `α = Σxᵢ/√n` | `ReserveState::alpha()` | `math/reserve_state.rs` |
| `\|\|w\|\|² = Σxᵢ²-(Σxᵢ)²/n` | `ReserveState::w_norm_squared()` | `math/reserve_state.rs` |
| check invariant (0.1% tol) | `Sphere::check_invariant()` | `math/sphere.rs` |
| Q64.64 fixed-point | `FixedPoint` (i128, FRAC_BITS=64) | `math/fixed_point.rs` |
| Tick structure | _Issue #4 (planned)_ | — |
| Torus invariant | _Issue #8 (planned)_ | — |

---

## 11. Mathematical Verification

All implemented formulas have been verified against the Paradigm Orbital paper:

| Formula | Paper | Our Code | Status |
|---------|-------|----------|--------|
| Sphere invariant | `Σ(r-xᵢ)²=r²` | `distance_squared == radius_squared` | ✅ Match |
| Price | `(r-xⱼ)/(r-xᵢ)` | `numerator.checked_div(denominator)` | ✅ Match |
| Equal price point | `r(1-1/√n)` | `radius * (1 - 1/sqrt(n))` | ✅ Match |
| Alpha | `Σxᵢ/√n` | `running_sum / sqrt(n)` | ✅ Match |
| W norm² | `Σxᵢ²-(Σxᵢ)²/n` | `running_sq_sum - sum²/n` | ✅ Match |
| O(1) distance² | `nr²-2rΣxᵢ+Σxᵢ²` | `n*r² - 2*r*sum + sq_sum` | ✅ Match |

> **Note on `||w||²`**: The paper presents `r²-(α-r√n)²` which is only valid ON
> the sphere surface (substitutes the invariant). Our formula `Σxᵢ²-(Σxᵢ)²/n`
> is the general decomposition that works in all cases. Both are equivalent on
> the sphere surface — verified algebraically.
