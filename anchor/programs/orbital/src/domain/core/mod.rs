// Core Context — AMM domain logic (sphere invariant, pool aggregate root, swap execution)
pub mod pool;
pub mod swap;

pub use pool::{
    compute_radius_from_deposit, derive_vault_pda, initialize_pool_reserves, update_caches,
    verify_invariant,
};
pub use swap::{compute_fee, compute_slippage_bps, execute_swap, SwapResult};

#[cfg(test)]
pub(crate) mod test_helpers {
    use anchor_lang::prelude::Pubkey;

    use crate::math::sphere::{Sphere, MAX_ASSETS};
    use crate::math::FixedPoint;
    use crate::state::PoolState;

    pub fn unique_pubkeys(n: usize) -> Vec<Pubkey> {
        (0..n).map(|_| Pubkey::new_unique()).collect()
    }

    pub fn make_pool(n: u8) -> PoolState {
        PoolState {
            bump: 0,
            authority: Pubkey::new_unique(),
            sphere: Sphere {
                radius: FixedPoint::zero(),
                n,
            },
            reserves: [FixedPoint::zero(); MAX_ASSETS],
            n_assets: n,
            token_mints: [Pubkey::default(); MAX_ASSETS],
            token_vaults: [Pubkey::default(); MAX_ASSETS],
            vault_bumps: [0u8; MAX_ASSETS],
            fee_rate_bps: 30,
            total_interior_liquidity: FixedPoint::zero(),
            total_boundary_liquidity: FixedPoint::zero(),
            alpha_cache: FixedPoint::zero(),
            w_norm_sq_cache: FixedPoint::zero(),
            tick_count: 0,
            is_active: true,
            total_volume: FixedPoint::zero(),
            total_fees: FixedPoint::zero(),
            created_at: 0,
            position_count: 0,
            _reserved: [0u8; 112],
        }
    }
}
