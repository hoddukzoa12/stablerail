//! Domain Events
//!
//! Anchor events emitted by instructions for off-chain indexing.
//! Each event captures the minimal data needed to reconstruct
//! state transitions without re-reading accounts.

use anchor_lang::prelude::*;

/// Emitted when a new pool is initialized via `initialize_pool`.
#[event]
pub struct PoolCreated {
    /// Pool account pubkey
    pub pool: Pubkey,
    /// Sphere radius (Q64.64 raw)
    pub radius: i128,
    /// Number of assets in the pool
    pub n_assets: u8,
    /// Token mints included in the pool
    pub token_mints: Vec<Pubkey>,
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
    /// Effective price: amount_in / amount_out (Q64.64 raw)
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
    /// Previous status (0 = Interior, 1 = Boundary)
    pub from_status: u8,
    /// New status (0 = Interior, 1 = Boundary)
    pub to_status: u8,
    /// Alpha norm at the moment of crossing (Q64.64 raw)
    pub alpha_at_crossing: i128,
    /// Unix timestamp
    pub timestamp: i64,
}
