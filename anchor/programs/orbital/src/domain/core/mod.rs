// Core Context — AMM domain logic (sphere invariant, pool aggregate root, swap execution)
pub mod pool;
pub mod swap;

pub use pool::{
    compute_radius_from_deposit, compute_radius_from_reserves, derive_vault_pda,
    initialize_pool_reserves, recompute_sphere, update_caches, verify_invariant,
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
            token_decimals: [0u8; MAX_ASSETS],
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
            _reserved: [0u8; 104],
        }
    }

    /// Generous epsilon for sqrt-derived comparisons (~2^-22)
    pub fn sqrt_epsilon() -> FixedPoint {
        FixedPoint::from_raw(1i128 << 42)
    }

    /// Initialize a pool with equal deposits and return it.
    pub fn init_pool(n: u8, deposit: i64) -> PoolState {
        let mut pool = make_pool(n);
        let deposit_fp = FixedPoint::from_int(deposit);
        let mints = unique_pubkeys(n as usize);
        let vaults = unique_pubkeys(n as usize);
        super::initialize_pool_reserves(&mut pool, deposit_fp, &mints, &vaults).unwrap();
        pool
    }
}
