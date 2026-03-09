# Orbital Settlement Protocol — Anchor Program

Solana-native implementation of the [Paradigm Orbital AMM](https://www.paradigm.xyz/2025/06/orbital) with an institutional settlement layer, built with [Anchor](https://www.anchor-lang.com/).

## Program Architecture

Single Anchor program with 4 DDD bounded-context modules:

| Module | Instructions |
|--------|-------------|
| **Core** | `initialize_pool`, `execute_swap` |
| **Liquidity** | `add_liquidity`, `remove_liquidity` |
| **Policy** | `create_policy`, `update_policy`, `manage_allowlist` |
| **Settlement** | `execute_settlement` |

## Deploying Your Own Program

### 1. Generate a new program keypair

```bash
cd anchor
solana-keygen new -o target/deploy/orbital-keypair.json
```

### 2. Get the new program ID

```bash
solana address -k target/deploy/orbital-keypair.json
```

### 3. Update the program ID

Update the program ID in these files:

- `anchor/Anchor.toml` — Update `orbital = "..."` under `[programs.devnet]`
- `anchor/programs/orbital/src/lib.rs` — Update `declare_id!("...")`

### 4. Build and deploy

```bash
# Build the program
anchor build

# Get devnet SOL for deployment (~2 SOL needed)
solana airdrop 2 --url devnet

# Deploy to devnet
anchor deploy --provider.cluster devnet
```

### 5. Regenerate the TypeScript client

```bash
cd ..
npm run codama:js
```

This updates the generated client code in `app/generated/orbital/` with your new program ID.

## Testing

Run the Anchor tests:

```bash
anchor test --skip-deploy
```
