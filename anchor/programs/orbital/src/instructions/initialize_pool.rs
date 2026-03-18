use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount};

use crate::domain::core::{derive_vault_pda, initialize_pool_reserves};
use crate::errors::OrbitalError;
use crate::events::PoolCreated;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};
use crate::state::PoolState;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitPoolParams {
    pub n_assets: u8,
    pub fee_rate_bps: u16,
    /// Per-asset deposit amount in token base units (e.g. 1_000_000 for 1 USDC)
    pub initial_deposit_per_asset: u64,
    /// Token mints for the pool; only first n_assets entries are used
    pub token_mints: [Pubkey; MAX_ASSETS],
}

/// Accounts for `initialize_pool`.
///
/// `remaining_accounts` layout (3 × n_assets):
///   [0..n)   = mint accounts       (read-only)
///   [n..2n)  = vault accounts      (writable, to be created via CPI)
///   [2n..3n) = authority ATAs      (writable, deposit source)
#[derive(Accounts)]
#[instruction(params: InitPoolParams)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = PoolState::SIZE,
        seeds = [b"pool", authority.key().as_ref()],
        bump,
    )]
    pub pool: Account<'info, PoolState>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, InitializePool<'info>>,
    params: InitPoolParams,
) -> Result<()> {
    let n = params.n_assets as usize;

    // ── Input validation ──
    require!(
        n >= 2 && n <= MAX_ASSETS,
        OrbitalError::InvalidAssetCount
    );
    require!(params.fee_rate_bps <= 10000, OrbitalError::InvalidFeeRate);
    require!(
        params.initial_deposit_per_asset > 0,
        OrbitalError::InvalidLiquidityAmount
    );

    // Validate remaining_accounts: need 3n accounts (mints + vaults + ATAs)
    let remaining = &ctx.remaining_accounts;
    require!(
        remaining.len() == 3 * n,
        OrbitalError::InvalidRemainingAccounts
    );

    // Reject duplicate mints early (before CPI loop to return correct error code)
    for i in 0..n {
        for j in (i + 1)..n {
            require!(
                params.token_mints[i] != params.token_mints[j],
                OrbitalError::DuplicateTokenMint
            );
        }
    }

    let pool = &mut ctx.accounts.pool;

    // ── Set basic fields ──
    pool.bump = ctx.bumps.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.n_assets = params.n_assets;
    pool.fee_rate_bps = params.fee_rate_bps;
    pool.is_active = true;
    pool.created_at = Clock::get()?.unix_timestamp;

    // ── Derive vault PDAs and create SPL token accounts ──
    let pool_key = pool.key();
    let mut vault_pubkeys = [Pubkey::default(); MAX_ASSETS];
    let mut vault_bumps = [0u8; MAX_ASSETS];

    let rent = &ctx.accounts.rent;
    let vault_space = TokenAccount::LEN;
    let vault_lamports = rent.minimum_balance(vault_space);

    // remaining_accounts offsets (matches doc comment layout)
    let mint_offset = 0;     // [0..n)
    let vault_offset = n;    // [n..2n)
    let ata_offset = 2 * n;  // [2n..3n)

    // Pool PDA seeds — loop-invariant, hoisted for clarity
    let pool_seeds: &[&[u8]] = &[
        b"pool",
        ctx.accounts.authority.key.as_ref(),
        &[pool.bump],
    ];

    // ── Read and validate token decimals from mint accounts ──
    let mut token_decimals_arr = [0u8; MAX_ASSETS];
    for i in 0..n {
        let mint_info = &remaining[mint_offset + i];
        let mint_data = mint_info.try_borrow_data()?;
        // SPL Mint layout: [mint_authority: 36B, supply: 8B, decimals: 1B, ...]
        require!(mint_data.len() >= 45, OrbitalError::InvalidRemainingAccounts);
        token_decimals_arr[i] = mint_data[44];
    }
    // MVP: require all pool tokens have the same decimals (stablecoin assumption)
    let pool_decimals = token_decimals_arr[0];
    for i in 1..n {
        require!(
            token_decimals_arr[i] == pool_decimals,
            OrbitalError::DecimalsMismatch
        );
    }
    pool.token_decimals = token_decimals_arr;

    for i in 0..n {
        let mint_info = &remaining[mint_offset + i];
        let vault_info = &remaining[vault_offset + i];

        // Verify mint matches params
        require!(
            *mint_info.key == params.token_mints[i],
            OrbitalError::InvalidTokenIndex
        );

        let (expected_vault, bump) =
            derive_vault_pda(&pool_key, mint_info.key, ctx.program_id);
        require!(
            *vault_info.key == expected_vault,
            OrbitalError::InvalidVaultAddress
        );

        vault_pubkeys[i] = expected_vault;
        vault_bumps[i] = bump;

        // CPI: create vault account with PDA seeds
        let vault_seeds: &[&[u8]] = &[
            b"vault",
            pool_key.as_ref(),
            mint_info.key.as_ref(),
            &[bump],
        ];
        system_program::create_account(
            CpiContext::new_with_signer(
                ctx.accounts.system_program.to_account_info(),
                system_program::CreateAccount {
                    from: ctx.accounts.authority.to_account_info(),
                    to: vault_info.clone(),
                },
                &[vault_seeds],
            ),
            vault_lamports,
            vault_space as u64,
            ctx.accounts.token_program.key,
        )?;

        // CPI: initialize token account (vault owned by pool PDA)
        token::initialize_account3(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::InitializeAccount3 {
                account: vault_info.clone(),
                mint: mint_info.clone(),
                authority: pool.to_account_info(),
            },
            &[pool_seeds],
        ))?;
    }

    // Store vault bumps
    pool.vault_bumps = vault_bumps;

    // ── Transfer initial deposits from authority ATAs to vaults ──
    for i in 0..n {
        let ata_info = &remaining[ata_offset + i];
        let vault_info = &remaining[vault_offset + i];

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                token::Transfer {
                    from: ata_info.clone(),
                    to: vault_info.clone(),
                    authority: ctx.accounts.authority.to_account_info(),
                },
            ),
            params.initial_deposit_per_asset,
        )?;
    }

    // ── Initialize reserves, sphere, and caches via domain logic ──
    // Normalize raw SPL amount to whole-token FixedPoint (e.g., 1_000_000 → FP(1.0) for 6 dec)
    let deposit_fp = FixedPoint::from_token_amount(params.initial_deposit_per_asset, pool_decimals)?;
    initialize_pool_reserves(pool, deposit_fp, &params.token_mints[..n], &vault_pubkeys[..n])?;

    // Record seed liquidity so close_pool can distinguish it from LP deposits.
    // After initialize_pool_reserves, total_interior_liquidity == seed amount.
    pool.seed_liquidity = pool.total_interior_liquidity;

    // ── Emit event ──
    emit!(PoolCreated {
        pool: pool.key(),
        authority: pool.authority,
        radius: pool.sphere.radius.raw,
        n_assets: pool.n_assets,
        token_mints: pool.token_mints,
        fee_rate_bps: pool.fee_rate_bps,
        timestamp: pool.created_at,
    });

    msg!(
        "Pool initialized: {} assets, {} bps fee, deposit {}",
        params.n_assets,
        params.fee_rate_bps,
        params.initial_deposit_per_asset
    );
    Ok(())
}
