// Core Context — AMM domain logic (sphere invariant, pool aggregate root, swap execution)
pub mod pool;
pub mod swap;

pub use pool::{
    compute_radius_from_deposit, derive_vault_pda, initialize_pool_reserves, update_caches,
    verify_invariant,
};
pub use swap::{compute_fee, compute_slippage_bps, execute_swap, SwapResult};
