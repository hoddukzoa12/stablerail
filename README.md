<p align="center">
  <img src="logo.png" alt="Orbital Settlement Protocol" width="160" />
</p>

<h1 align="center">Orbital Settlement Protocol</h1>

<p align="center">
  <strong>Paradigm's Orbital AMM on Solana — with institutional settlement</strong>
</p>

<p align="center">
  <a href="https://www.paradigm.xyz/2025/06/orbital">Paper</a> ·
  <a href="https://solana.com/">Solana</a> ·
  <a href="https://www.anchor-lang.com/">Anchor</a> ·
  <a href="https://dorahacks.io/">StableHacks 2026</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Solana-Devnet-blue?logo=solana" />
  <img src="https://img.shields.io/badge/Anchor-0.31.1-purple" />
  <img src="https://img.shields.io/badge/Next.js-16-black?logo=next.js" />
  <img src="https://img.shields.io/badge/License-MIT-green" />
</p>

---

## What is Orbital?

Orbital is a next-generation multi-asset stablecoin AMM designed by [Paradigm](https://www.paradigm.xyz/2025/06/orbital). It combines the best of Curve (multi-asset pools) and Uniswap V3 (concentrated liquidity) while adding a novel **depeg isolation** mechanism.

This repository is the **first Solana-native implementation** of the Orbital AMM, extended with a **permissioned institutional settlement layer** for regulated entities.

### Key Innovations

| Feature | Description |
|---------|-------------|
| **Sphere Invariant** | `\|\|r - x\|\|² = r²` — geometric invariant enabling N-asset pools on a hypersphere |
| **Nested Ticks** | Spherical cap-based concentrated liquidity with per-tick reserves |
| **Depeg Isolation** | When an asset depegs, its tick flips to `Boundary` — isolating risk from other LPs |
| **Trade Segmentation** | Multi-tick swap execution with boundary detection and automatic tick crossing |
| **Institutional Settlement** | Policy engine, allowlists, daily volume limits, and on-chain audit trails |

---

## Architecture

Single Anchor program with 4 bounded contexts as Rust modules:

```
orbital/
├── math/               # Core math engine
│   ├── sphere.rs       # Sphere invariant, price, equal-price point
│   ├── torus.rs        # Tick crossing detection, delta-to-boundary
│   ├── tick.rs         # k bounds, x_min/x_max, capital efficiency
│   ├── newton.rs       # Analytical swap solver (quadratic)
│   ├── fixed_point.rs  # Q64.64 fixed-point arithmetic (i128)
│   └── reserve_state.rs # O(1) invariant verification
│
├── domain/             # Business logic
│   ├── core/           # Pool operations, swap math
│   ├── liquidity/      # LP position management
│   ├── settlement/     # Institutional settlement orchestration
│   └── policy/         # Access control, compliance
│
├── instructions/       # On-chain instruction handlers
│   ├── initialize_pool     # Create N-asset pool with sphere invariant
│   ├── execute_swap        # Multi-segment swap with tick crossing
│   ├── create_tick         # Deploy concentrated liquidity tick
│   ├── add_liquidity       # Deposit to full-range or tick position
│   ├── remove_liquidity    # Withdraw with boundary-aware logic
│   ├── create_policy       # Define settlement policy
│   ├── update_policy       # Modify policy parameters
│   ├── manage_allowlist    # Add/remove institutional participants
│   ├── execute_settlement  # Policy-checked institutional swap
│   └── close_pool          # Authority-only pool shutdown
│
├── state/              # Account definitions (PDA)
│   ├── pool, position, tick
│   ├── policy, allowlist
│   └── settlement, audit_entry
│
├── errors.rs           # Program error codes
└── events.rs           # CPI event definitions
```

---

## Math Reference

Based on the [Paradigm Orbital paper](https://www.paradigm.xyz/2025/06/orbital):

| Formula | Description | Implementation |
|---------|-------------|----------------|
| `Σ(r - xᵢ)² = r²` | Sphere invariant | `sphere.rs` |
| `(r - xⱼ) / (r - xᵢ)` | Marginal price | `sphere.rs` |
| `q = r(1 - 1/√n)` | Equal price point | `sphere.rs` |
| `α = Σxᵢ / √n` | Alpha (torus coordinate) | `torus.rs` |
| `k_min = r(√n - 1)` | Minimum tick bound | `tick.rs` |
| `k_max = r(n-1) / √n` | Maximum tick bound | `tick.rs` |
| `D = √(k²n - n((n-1)r - k√n)²)` | Tick discriminant | `tick.rs` |
| `x_min = (k√n - D) / n` | Lower reserve bound | `tick.rs` |
| `x_max = min(r, (k√n + D) / n)` | Upper reserve bound | `tick.rs` |
| `s(α) = √(r² - (α - r√n)²)` | Boundary sphere radius | `torus.rs` |
| `d_out = -b + √(b² + 2ad - d²)` | Analytical swap output | `newton.rs` |

**Precision**: Q64.64 fixed-point (`i128` backing, 64 fractional bits) for all on-chain math.

---

## Frontend

Next.js 16 app with real-time Solana devnet integration:

| Page | Features |
|------|----------|
| **Swap** | Token selection, real-time quote with tick-aware calculator, slippage settings |
| **Dashboard** | Pool overview, reserve chart, LP positions, tick selector for concentrated liquidity |
| **Settlement** | Policy-compliant institutional swap form, compliance preview, audit trail table |
| **Faucet** | Devnet SPL token faucet (USDC, USDT, PYUSD) |
| **Admin** | Pool initialization, tick creation, policy management |

---

## Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) 1.75+
- [Solana CLI](https://docs.solanalabs.com/cli/install) 1.18+
- [Anchor](https://www.anchor-lang.com/docs/installation) 0.31.1
- [Node.js](https://nodejs.org/) 20+

### Build & Test

```bash
# Install dependencies
npm install

# Build Anchor program
npm run anchor-build

# Run on-chain tests
npm run anchor-test

# Start frontend dev server
npm run dev
```

### Deploy to Devnet

```bash
# Deploy program
npm run deploy:devnet

# Bootstrap pool + ticks with demo liquidity
npm run bootstrap:devnet
```

### Program ID

```
BZDXfJTBpH9ZMo2dz57BFKGNw4FYFCDr1KaUUkFtfRVD
```

---

## Demo Configuration (Devnet)

3-asset pool: **USDC · USDT · PYUSD**

| Parameter | Value |
|-----------|-------|
| Pool TVL | $150M (demo tokens) |
| Fee | 1 bps (0.01%) |
| Assets | 3 (n = 3) |
| Ticks | 5 concentrated ticks (3.5× – 18× concentration) |
| Tick TVL | $49M/asset across ticks |

---

## Implementation Status

Comprehensive analysis against the [Paradigm Orbital paper](https://www.paradigm.xyz/2025/06/orbital):

| Paper Concept | Status | Notes |
|---------------|--------|-------|
| Sphere invariant | ✅ Complete | Both O(n) and O(1) verification paths |
| Multi-asset pools (n ≤ 8) | ✅ Complete | Parameterized n with Q64.64 precision |
| Price formula | ✅ Complete | Marginal price, equal-price point |
| Tick math (k bounds, x_min/x_max) | ✅ Complete | All formulas verified against paper |
| Trade segmentation loop | ✅ Complete | While loop with boundary detection and tick flip |
| Analytical swap solver | ✅ Complete | Closed-form quadratic (CU-optimized) |
| Depeg isolation (tick flip) | ✅ Complete | Interior → Boundary status transition |
| Torus tick-crossing detection | ⚠️ Partial | Alpha-based detection works; full torus consolidation deferred |
| Tick consolidation (r_c = Σrᵢ) | ⚠️ Planned | Individual tick tracking, no aggregation |
| Virtual reserve amplification | ⚠️ Planned | x_min computed but not used in swap math ([#36](https://github.com/hoddukzoa12/stablerail/issues/36)) |
| Per-tick fee distribution | ⚠️ Planned | Fees tracked globally, no per-tick LP harvest |

> **Note**: The MVP prioritizes correctness of the sphere invariant and trade segmentation infrastructure. Virtual reserve amplification (the mechanism that converts concentrated liquidity into reduced slippage) is tracked in [issue #36](https://github.com/hoddukzoa12/stablerail/issues/36) for post-MVP implementation.

---

## Project Structure

```
stablerail/
├── anchor/                 # Solana program (Rust/Anchor)
│   ├── programs/orbital/   # Main program source
│   └── Anchor.toml         # Anchor configuration
│
├── app/                    # Next.js frontend
│   ├── components/         # React components (swap, dashboard, settlement)
│   ├── hooks/              # Custom hooks (usePool, useSwapQuote, usePoolTicks)
│   └── lib/                # Math library, config, deserializers
│
├── scripts/                # Deployment & bootstrap scripts
│   ├── deploy-devnet.sh    # Program deployment
│   ├── bootstrap-pool.ts   # Pool + tick + liquidity setup
│   └── create-demo-ticks.ts # Demo tick creation
│
└── docs/                   # Documentation
    ├── Orbital_Math_Reference.md
    ├── Orbital_DDD_Architecture.md
    ├── Orbital_Settlement_Protocol_PRD_v2.1.md
    └── DESIGN_SYSTEM.md
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Smart Contract | Rust, Anchor 0.31.1 |
| Math Engine | Custom Q64.64 fixed-point (i128) |
| Frontend | Next.js 16, React 19, TypeScript |
| Styling | Tailwind CSS 4 |
| Charts | Recharts |
| Wallet | @solana/kit, Phantom |
| Network | Solana Devnet |

---

## Contributing

This project was built for [StableHacks 2026](https://dorahacks.io/). Contributions, issues, and discussions are welcome.

---

## References

- [Paradigm — Orbital: A Multi-Asset Automated Market Maker](https://www.paradigm.xyz/2025/06/orbital)
- [Anchor Framework](https://www.anchor-lang.com/)
- [Solana Documentation](https://docs.solanalabs.com/)

---

## License

MIT
