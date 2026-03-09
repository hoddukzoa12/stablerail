use anchor_lang::prelude::*;

#[error_code]
pub enum OrbitalError {
    // ── Math Errors ──
    #[msg("Math overflow in fixed-point operation")]
    MathOverflow,

    #[msg("Division by zero")]
    DivisionByZero,

    #[msg("Square root of negative number")]
    SqrtNegative,

    // ── Invariant Errors ──
    #[msg("Sphere invariant violated: ||r - x||^2 != r^2")]
    InvariantViolation,

    #[msg("Torus invariant computation failed")]
    TorusInvariantError,

    // ── Pool Errors ──
    #[msg("Pool already initialized")]
    PoolAlreadyInitialized,

    #[msg("Invalid number of assets (must be 2..=8)")]
    InvalidAssetCount,

    #[msg("Invalid fee rate")]
    InvalidFeeRate,

    #[msg("Insufficient liquidity for swap")]
    InsufficientLiquidity,

    #[msg("Pool is not active")]
    PoolNotActive,

    #[msg("Slippage tolerance exceeded")]
    SlippageExceeded,

    #[msg("Same token swap not allowed")]
    SameTokenSwap,

    #[msg("Invalid token index")]
    InvalidTokenIndex,

    // ── Tick Errors ──
    #[msg("Invalid tick bound k")]
    InvalidTickBound,

    #[msg("Tick crossing detected but not handled")]
    UnhandledTickCrossing,

    // ── Newton Solver Errors ──
    #[msg("Newton solver diverged")]
    NewtonDivergence,

    #[msg("Solver did not converge within max iterations")]
    SolverDidNotConverge,

    // ── Liquidity Errors ──
    #[msg("Invalid liquidity amount")]
    InvalidLiquidityAmount,

    #[msg("Trade amount must be non-negative")]
    NegativeTradeAmount,

    #[msg("Position not found")]
    PositionNotFound,

    #[msg("Insufficient position balance")]
    InsufficientPositionBalance,

    // ── Policy Errors ──
    #[msg("Unauthorized: caller not in allowlist")]
    Unauthorized,

    #[msg("Policy not found")]
    PolicyNotFound,

    #[msg("Trade exceeds policy limit")]
    PolicyLimitExceeded,

    #[msg("Allowlist is full")]
    AllowlistFull,

    #[msg("Address already in allowlist")]
    AlreadyInAllowlist,

    #[msg("Address not in allowlist")]
    NotInAllowlist,

    // ── Settlement Errors ──
    #[msg("Settlement policy check failed")]
    SettlementPolicyViolation,

    #[msg("Invalid settlement amount")]
    InvalidSettlementAmount,

    #[msg("Settlement audit trail creation failed")]
    AuditTrailError,

    // ── Pool Validation (appended for code stability) ──
    #[msg("Duplicate token mint in pool")]
    DuplicateTokenMint,
}
