use anchor_lang::prelude::*;

pub mod domain;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w");

#[program]
pub mod orbital {
    use super::*;

    // ═══════════════════════════════════════════
    //  Core Context — AMM Math Engine
    // ═══════════════════════════════════════════

    pub fn initialize_pool<'info>(
        ctx: Context<'_, '_, 'info, 'info, InitializePool<'info>>,
        params: InitPoolParams,
    ) -> Result<()> {
        instructions::initialize_pool::handler(ctx, params)
    }

    pub fn execute_swap<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteSwap<'info>>,
        params: SwapParams,
    ) -> Result<()> {
        instructions::execute_swap::handler(ctx, params)
    }

    // ═══════════════════════════════════════════
    //  Liquidity Context — LP Position Management
    // ═══════════════════════════════════════════

    pub fn add_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, AddLiquidity<'info>>,
        params: AddLiquidityParams,
    ) -> Result<()> {
        instructions::add_liquidity::handler(ctx, params)
    }

    pub fn remove_liquidity<'info>(
        ctx: Context<'_, '_, 'info, 'info, RemoveLiquidity<'info>>,
        params: RemoveLiquidityParams,
    ) -> Result<()> {
        instructions::remove_liquidity::handler(ctx, params)
    }

    // ═══════════════════════════════════════════
    //  Policy Context — Access Control & Policy
    // ═══════════════════════════════════════════

    pub fn create_policy(ctx: Context<CreatePolicy>, params: CreatePolicyParams) -> Result<()> {
        instructions::create_policy::handler(ctx, params)
    }

    pub fn update_policy(ctx: Context<UpdatePolicy>, params: UpdatePolicyParams) -> Result<()> {
        instructions::update_policy::handler(ctx, params)
    }

    pub fn manage_allowlist(
        ctx: Context<ManageAllowlist>,
        params: ManageAllowlistParams,
    ) -> Result<()> {
        instructions::manage_allowlist::handler(ctx, params)
    }

    // ═══════════════════════════════════════════
    //  Settlement Context — Institutional Settlement
    // ═══════════════════════════════════════════

    pub fn execute_settlement<'info>(
        ctx: Context<'_, '_, 'info, 'info, ExecuteSettlement<'info>>,
        params: ExecuteSettlementParams,
    ) -> Result<()> {
        instructions::execute_settlement::handler(ctx, params)
    }
}
