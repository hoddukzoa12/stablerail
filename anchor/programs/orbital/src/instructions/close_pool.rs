use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};

use crate::errors::OrbitalError;
use crate::state::PoolState;

/// Accounts for `close_pool`.
///
/// Closes the pool PDA and all associated vault token accounts,
/// returning lamports and remaining tokens to the authority.
///
/// `remaining_accounts` layout (2 * n_assets):
///   [0..n)   = vault accounts       (writable, to be closed)
///   [n..2n)  = authority ATAs        (writable, receive vault tokens)
#[derive(Accounts)]
pub struct ClosePool<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"pool", authority.key().as_ref()],
        bump = pool.bump,
        close = authority,
    )]
    pub pool: Account<'info, PoolState>,

    pub token_program: Program<'info, Token>,
}

pub fn handler<'info>(
    ctx: Context<'_, '_, 'info, 'info, ClosePool<'info>>,
) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let n = pool.n_assets as usize;

    // Guard: reject close if any LP positions are outstanding.
    // This preserves the emergency-exit guarantee for LPs (see remove_liquidity.rs).
    //
    // TODO(post-hackathon): This guard is unreachable — initial seed liquidity
    // (per_asset_deposit * n) has no Position PDA and no burn path, so
    // total_interior_liquidity can never reach zero. Fix: add `initial_liquidity`
    // field to PoolState and compare against it instead of zero.
    // Tracked: https://github.com/hoddukzoa12/stablerail/issues/47
    require!(
        pool.total_interior_liquidity.is_zero(),
        OrbitalError::PoolNotEmpty
    );


    let remaining = &ctx.remaining_accounts;
    require!(
        remaining.len() == 2 * n,
        OrbitalError::InvalidRemainingAccounts
    );

    // Pool PDA signs for vault close_account CPI
    let authority_key = pool.authority;
    let pool_bump = pool.bump;
    let pool_seeds: &[&[u8]] = &[b"pool", authority_key.as_ref(), &[pool_bump]];

    // Drain and close each vault
    for i in 0..n {
        let vault_info = &remaining[i];
        let dest_ata_info = &remaining[n + i];

        // Validate vault address matches pool state
        require!(
            *vault_info.key == pool.token_vaults[i],
            OrbitalError::InvalidVaultAddress
        );

        // Validate destination ATA is owned by authority (defense-in-depth,
        // mirrors execute_settlement.rs pattern for consistent security posture)
        {
            let ata_data = dest_ata_info.try_borrow_data()?;
            require!(ata_data.len() >= 64, OrbitalError::InvalidRemainingAccounts);
            // SPL Token Account layout: [mint 32B][owner 32B]...
            require!(
                ata_data[32..64] == ctx.accounts.authority.key().to_bytes(),
                OrbitalError::Unauthorized
            );
        }


        // Step 1: Transfer all tokens from vault to authority ATA
        let vault_data = vault_info.try_borrow_data()?;
        // SPL Token Account layout: [mint 32B][owner 32B][amount 8B]
        let vault_balance = u64::from_le_bytes(vault_data[64..72].try_into().unwrap());
        drop(vault_data);

        if vault_balance > 0 {
            token::transfer(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    token::Transfer {
                        from: vault_info.clone(),
                        to: dest_ata_info.clone(),
                        authority: pool.to_account_info(),
                    },
                    &[pool_seeds],
                ),
                vault_balance,
            )?;
        }

        // Step 2: Close empty vault (rent lamports → authority)
        token::close_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::CloseAccount {
                account: vault_info.clone(),
                destination: ctx.accounts.authority.to_account_info(),
                authority: pool.to_account_info(),
            },
            &[pool_seeds],
        ))?;
    }

    msg!(
        "Pool closed: {} assets, authority={}",
        n,
        authority_key
    );

    Ok(())
}
