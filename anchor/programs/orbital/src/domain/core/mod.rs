// Core Context — AMM domain logic (sphere invariant, pool aggregate root)
pub mod pool;

pub use pool::{
    compute_radius_from_deposit, derive_vault_pda, initialize_pool_reserves, update_caches,
    verify_invariant,
};
