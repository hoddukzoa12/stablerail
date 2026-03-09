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
