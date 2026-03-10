//! Domain Events
//!
//! Anchor events emitted by instructions for off-chain indexing.
//! Each event captures the minimal data needed to reconstruct
//! state transitions without re-reading accounts.

use anchor_lang::prelude::*;

use crate::math::sphere::MAX_ASSETS;
use crate::state::TickStatus;

/// Emitted when a new pool is initialized via `initialize_pool`.
#[event]
pub struct PoolCreated {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// Pool authority (PDA seed: ["pool", authority])
    pub authority: Pubkey,
    /// Sphere radius (Q64.64 raw)
    pub radius: i128,
    /// Number of active assets in the pool (valid entries in token_mints)
    pub n_assets: u8,
    /// Token mints (fixed-size; only first n_assets entries are valid)
    pub token_mints: [Pubkey; MAX_ASSETS],
    /// Fee rate in basis points
    pub fee_rate_bps: u16,
    /// Unix timestamp of creation
    pub timestamp: i64,
}

/// Emitted when a swap is executed via `execute_swap`.
#[event]
pub struct SwapExecuted {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// Mint of the token sent in
    pub token_in: Pubkey,
    /// Mint of the token received
    pub token_out: Pubkey,
    /// Amount deposited (Q64.64 raw)
    pub amount_in: i128,
    /// Amount withdrawn (Q64.64 raw)
    pub amount_out: i128,
    /// Execution price (average fill): amount_in / amount_out (Q64.64 raw)
    pub price: i128,
    /// Slippage in basis points
    pub slippage_bps: u16,
    /// Unix timestamp
    pub timestamp: i64,
}

/// Emitted when a tick crosses between Interior and Boundary status
/// during a swap.
#[event]
pub struct TickCrossed {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// Tick account pubkey
    pub tick: Pubkey,
    /// Previous tick status
    pub from_status: TickStatus,
    /// New tick status
    pub to_status: TickStatus,
    /// Alpha norm at the moment of crossing (Q64.64 raw)
    pub alpha_at_crossing: i128,
    /// Unix timestamp
    pub timestamp: i64,
}

/// Emitted when liquidity is added to a pool via `add_liquidity`.
#[event]
pub struct LiquidityAdded {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// LP provider pubkey
    pub provider: Pubkey,
    /// Position account pubkey
    pub position: Pubkey,
    /// Per-token deposit amounts in base units (only first n_assets valid)
    pub amounts: [u64; MAX_ASSETS],
    /// Liquidity units assigned to position (Q64.64 raw)
    pub liquidity: i128,
    /// New sphere radius after deposit (Q64.64 raw)
    pub new_radius: i128,
    /// Number of active assets in the pool
    pub n_assets: u8,
    /// Unix timestamp
    pub timestamp: i64,
}

/// Emitted when liquidity is removed from a pool via `remove_liquidity`.
#[event]
pub struct LiquidityRemoved {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// LP provider pubkey
    pub provider: Pubkey,
    /// Position account pubkey
    pub position: Pubkey,
    /// Per-token returned amounts in base units (only first n_assets valid)
    pub amounts: [u64; MAX_ASSETS],
    /// Liquidity units removed (Q64.64 raw)
    pub liquidity_removed: i128,
    /// Remaining liquidity in the position (Q64.64 raw)
    pub remaining_liquidity: i128,
    /// New sphere radius after withdrawal (Q64.64 raw)
    pub new_radius: i128,
    /// Number of active assets in the pool
    pub n_assets: u8,
    /// Unix timestamp
    pub timestamp: i64,
}
