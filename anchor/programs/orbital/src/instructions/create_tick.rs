use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::events::TickCreated;
use crate::math::sphere::MAX_ASSETS;
use crate::math::tick::Tick;
use crate::math::FixedPoint;
use crate::state::{PoolState, TickState, TickStatus};

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct CreateTickParams {
    /// Plane constant k as Q64.64 raw value.
    /// Must satisfy k_min < k < k_max for the pool's current sphere.
    pub k_raw: i128,
}

/// Accounts for `create_tick`.
///
/// Creates a new tick (spherical cap) for concentrated liquidity.
/// The tick is derived from PDA: ["tick", pool, k_raw_le_bytes].
/// Using k_raw in the seed enforces uniqueness — duplicate k values
/// cause PDA collision and Anchor's `init` will fail.
#[derive(Accounts)]
#[instruction(params: CreateTickParams)]
pub struct CreateTick<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", pool.authority.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, PoolState>>,

    #[account(
        init,
        payer = creator,
        space = TickState::SIZE,
        seeds = [
            b"tick",
            pool.key().as_ref(),
            &params.k_raw.to_le_bytes(),
        ],
        bump,
    )]
    pub tick: Box<Account<'info, TickState>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<CreateTick>, params: CreateTickParams) -> Result<()> {
    let pool = &ctx.accounts.pool;

    // ── Validation ──
    require!(pool.is_active, OrbitalError::PoolNotActive);

    // Only pool authority can create ticks (prevents griefing via tick spam
    // that would brick swaps by exceeding Solana's per-tx account limit).
    require!(
        ctx.accounts.creator.key() == pool.authority,
        OrbitalError::Unauthorized
    );

    // Cap tick count to prevent bricking swaps.
    // Solana tx = 1232 bytes; each remaining_account = 32 bytes.
    // execute_swap uses 4 fixed remaining_accounts (vaults + ATAs),
    // leaving room for ~30 ticks. We cap at 16 for safety margin.
    const MAX_TICKS: u16 = 16;
    require!(
        pool.tick_count < MAX_TICKS,
        OrbitalError::MaxTicksReached
    );

    // Convert raw k to FixedPoint and validate bounds via Tick::new
    let k = FixedPoint::from_raw(params.k_raw);
    let tick_math = Tick::new(k, &pool.sphere)?;

    // ── Populate TickState ──
    let tick = &mut ctx.accounts.tick;
    tick.bump = ctx.bumps.tick;
    tick.pool = pool.key();
    tick.k = k;
    tick.status = TickStatus::Interior;
    tick.liquidity = FixedPoint::zero();
    tick.sphere_radius = tick_math.boundary_sphere_radius;
    tick.depeg_price = tick_math.depeg_price;
    tick.x_min = tick_math.x_min;
    tick.x_max = tick_math.x_max;
    tick.capital_efficiency = tick_math.capital_efficiency;
    tick.owner = ctx.accounts.creator.key();
    tick.reserves = [FixedPoint::zero(); MAX_ASSETS];
    tick._reserved = [0u8; 32];

    let clock = Clock::get()?;
    tick.created_at = clock.unix_timestamp;

    // ── Increment tick counter ──
    let pool = &mut ctx.accounts.pool;
    pool.tick_count = pool
        .tick_count
        .checked_add(1)
        .ok_or(OrbitalError::MathOverflow)?;

    // ── Emit event ──
    emit!(TickCreated {
        pool: pool.key(),
        tick: ctx.accounts.tick.key(),
        k: k.raw,
        x_min: tick_math.x_min.raw,
        x_max: tick_math.x_max.raw,
        depeg_price: tick_math.depeg_price.raw,
        capital_efficiency: tick_math.capital_efficiency.raw,
        timestamp: clock.unix_timestamp,
    });

    msg!(
        "Tick created: k={}, x_min={}, x_max={}, efficiency={}",
        k,
        tick_math.x_min,
        tick_math.x_max,
        tick_math.capital_efficiency,
    );

    Ok(())
}
