use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::domain::core::{derive_vault_pda, initialize_pool_reserves};
use crate::errors::OrbitalError;
use crate::math::{sphere::MAX_ASSETS, FixedPoint};
use crate::state::PoolState;

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitPoolParams {
    pub n_assets: u8,
    pub fee_rate_bps: u16,
    /// Per-asset deposit amount in token base units (e.g. lamports for SOL)
    pub initial_deposit_per_asset: u64,
    /// Token mints for the pool; only first n_assets entries are used
    pub token_mints: [Pubkey; MAX_ASSETS],
}

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
}

pub fn handler(ctx: Context<InitializePool>, params: InitPoolParams) -> Result<()> {
    let pool = &mut ctx.accounts.pool;

    require!(
        params.n_assets >= 2 && params.n_assets as usize <= MAX_ASSETS,
        OrbitalError::InvalidAssetCount
    );
    require!(params.fee_rate_bps <= 10000, OrbitalError::InvalidFeeRate);

    // Set non-zero fields; remaining fields are zero-initialized by Anchor's `init`.
    pool.bump = ctx.bumps.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.n_assets = params.n_assets;
    pool.fee_rate_bps = params.fee_rate_bps;
    pool.is_active = true;
    pool.created_at = Clock::get()?.unix_timestamp;

    // Derive vault PDAs for each token mint
    let n = params.n_assets as usize;
    let pool_key = pool.key();
    let mut vault_pubkeys = [Pubkey::default(); MAX_ASSETS];
    for i in 0..n {
        let (vault_pda, _bump) = derive_vault_pda(&pool_key, &params.token_mints[i], ctx.program_id);
        vault_pubkeys[i] = vault_pda;
    }

    // Initialize reserves, sphere, and caches via domain logic
    let deposit_fp = FixedPoint::checked_from_u64(params.initial_deposit_per_asset)?;
    initialize_pool_reserves(pool, deposit_fp, &params.token_mints[..n], &vault_pubkeys[..n])?;

    msg!(
        "Pool initialized: {} assets, {} bps fee, deposit {}",
        params.n_assets,
        params.fee_rate_bps,
        params.initial_deposit_per_asset
    );
    Ok(())
}
