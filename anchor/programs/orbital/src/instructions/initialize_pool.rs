use anchor_lang::prelude::*;
use anchor_spl::token::Token;

use crate::state::PoolState;
use crate::errors::OrbitalError;
use crate::math::{sphere::MAX_ASSETS, FixedPoint, Sphere};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct InitPoolParams {
    pub n_assets: u8,
    pub fee_rate_bps: u16,
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

    msg!("Pool initialized: {} assets, {} bps fee", params.n_assets, params.fee_rate_bps);
    Ok(())
}
