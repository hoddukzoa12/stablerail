// Core Context Domain Logic
// Pool aggregate root, SwapCalculator, NewtonSolver, TickConsolidator
pub mod pool;

pub use pool::{
    compute_radius_from_deposit, derive_vault_pda, initialize_pool_reserves, update_caches,
    verify_invariant,
};
