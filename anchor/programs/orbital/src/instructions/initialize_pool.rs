use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::state::PoolState;
use crate::errors::OrbitalError;
use crate::math::{sphere::MAX_ASSETS, FixedPoint, Sphere};

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

    pool.bump = ctx.bumps.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.n_assets = params.n_assets;
    pool.fee_rate_bps = params.fee_rate_bps;
    pool.is_active = true;
    pool.tick_count = 0;
    pool.total_volume = FixedPoint::zero();
    pool.total_fees = FixedPoint::zero();
    pool.total_interior_liquidity = FixedPoint::zero();
    pool.total_boundary_liquidity = FixedPoint::zero();
    pool.alpha_cache = FixedPoint::zero();
    pool.w_norm_sq_cache = FixedPoint::zero();
    pool.sphere = Sphere { radius: FixedPoint::zero(), n: params.n_assets };
    pool.reserves = [FixedPoint::zero(); MAX_ASSETS];
    pool.token_mints = [Pubkey::default(); MAX_ASSETS];
    pool.token_vaults = [Pubkey::default(); MAX_ASSETS];
    pool.position_count = 0;
    pool._reserved = [0u8; 120];

    let clock = Clock::get()?;
    pool.created_at = clock.unix_timestamp;

    // Initialize reserves and sphere via domain logic
    let deposit_fp = FixedPoint::checked_from_u64(params.initial_deposit_per_asset)?;
    let n = params.n_assets as usize;
    let pool_key = pool.key();
    let program_id = ctx.program_id;

    // Derive vault PDAs for each token mint
    let mut vault_pubkeys = [Pubkey::default(); MAX_ASSETS];
    for i in 0..n {
        let (vault_pda, _bump) = crate::domain::core::pool::derive_vault_pda(
            &pool_key,
            &params.token_mints[i],
            program_id,
        );
        vault_pubkeys[i] = vault_pda;
    }

    crate::domain::core::pool::initialize_pool_reserves(
        pool,
        deposit_fp,
        &params.token_mints[..n],
        &vault_pubkeys[..n],
    )?;

    msg!("Pool initialized: {} assets, {} bps fee, deposit {}", params.n_assets, params.fee_rate_bps, params.initial_deposit_per_asset);
    Ok(())
}
