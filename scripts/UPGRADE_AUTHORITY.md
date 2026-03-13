# Upgrade Authority

## Current Posture (Devnet — Hackathon)

- **Upgrade authority**: Deployer wallet (`~/.config/solana/id.json`)
- Acceptable for devnet — enables rapid iteration during the hackathon sprint
- The deployer is also the pool authority and the initial settlement executor

## Mainnet Recommendations

### Option A: Multisig (Recommended)

Transfer upgrade authority to a multisig program (e.g., Squads Protocol):

```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority <MULTISIG_ADDRESS>
```

Benefits: No single key can unilaterally upgrade the program.

### Option B: Immutable

Make the program non-upgradeable:

```bash
solana program set-upgrade-authority <PROGRAM_ID> --final
```

Benefits: Maximum trust — code cannot change. Risk: Cannot patch bugs.

## Key Security Properties

1. **No admin drain while LPs exist**: `close_pool` requires `total_interior_liquidity == 0` before draining vaults
2. **Non-custodial during operation**: Funds move through swap/liquidity CPI paths; `close_pool` only available after all LPs withdraw
3. **Fee model**: Fees accrue as LP liquidity (Curve-style), no separate extraction
4. **Settlement guardrails**: Allowlist + trade limits + daily volume caps + audit trail
