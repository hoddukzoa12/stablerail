use anchor_lang::prelude::*;

use crate::state::{AllowlistState, PolicyState, allowlist::MAX_ALLOWLIST_SIZE};
use crate::errors::OrbitalError;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum AllowlistAction {
    Add,
    Remove,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct ManageAllowlistParams {
    pub action: AllowlistAction,
    pub address: Pubkey,
}

#[derive(Accounts)]
pub struct ManageAllowlist<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        has_one = authority @ OrbitalError::Unauthorized,
    )]
    pub policy: Account<'info, PolicyState>,

    #[account(
        init_if_needed,
        payer = authority,
        space = AllowlistState::SIZE,
        seeds = [b"allowlist", policy.key().as_ref()],
        bump,
    )]
    pub allowlist: Box<Account<'info, AllowlistState>>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ManageAllowlist>, params: ManageAllowlistParams) -> Result<()> {
    let allowlist = &mut ctx.accounts.allowlist;

    if allowlist.policy == Pubkey::default() {
        allowlist.bump = ctx.bumps.allowlist;
        allowlist.policy = ctx.accounts.policy.key();
        allowlist.authority = ctx.accounts.authority.key();
        allowlist.count = 0;
        allowlist.addresses = [Pubkey::default(); MAX_ALLOWLIST_SIZE];
        allowlist._reserved = [0u8; 64];
    }

    match params.action {
        AllowlistAction::Add => {
            allowlist.add(params.address)?;
            msg!("Added {} to allowlist", params.address);
        }
        AllowlistAction::Remove => {
            allowlist.remove(&params.address)?;
            msg!("Removed {} from allowlist", params.address);
        }
    }

    Ok(())
}
