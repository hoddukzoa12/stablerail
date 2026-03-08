// DDD Domain Logic — Pure Rust business rules
// Contexts are logically separated here while instructions/state remain flat for Anchor compatibility
pub mod core;
pub mod liquidity;
pub mod settlement;
pub mod policy;
