//! Shared tick account helpers for instruction handlers.
//!
//! Centralizes TickState deserialization/serialization with program ownership
//! and discriminator validation. Used by add_liquidity, remove_liquidity,
//! execute_swap, and any future tick-aware instructions.

use anchor_lang::prelude::*;

use crate::errors::OrbitalError;
use crate::state::TickState;

/// Deserialize a TickState from an AccountInfo (read-only).
///
/// Validates program ownership and Anchor discriminator to prevent forged accounts.
/// Use [`load_tick_state_mut`] when you intend to write back via [`save_tick_state`].
pub fn load_tick_state(acc: &AccountInfo) -> Result<TickState> {
    require!(acc.owner == &crate::ID, OrbitalError::InvalidTickAccount);

    let data = acc.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    TickState::try_deserialize(&mut slice)
        .map_err(|_| OrbitalError::InvalidTickAccount.into())
}

/// Deserialize a TickState from an AccountInfo, requiring writable access.
///
/// Same as [`load_tick_state`] but also validates the account is writable,
/// failing early with a clear error instead of an opaque `AccountBorrowFailed`
/// when [`save_tick_state`] is called later.
pub fn load_tick_state_mut(acc: &AccountInfo) -> Result<TickState> {
    require!(acc.owner == &crate::ID, OrbitalError::InvalidTickAccount);
    require!(acc.is_writable, OrbitalError::InvalidTickAccount);

    let data = acc.try_borrow_data()?;
    let mut slice: &[u8] = &data;
    TickState::try_deserialize(&mut slice)
        .map_err(|_| OrbitalError::InvalidTickAccount.into())
}

/// Serialize TickState back into AccountInfo (preserving 8-byte Anchor discriminator).
pub fn save_tick_state(acc: &AccountInfo, tick: &TickState) -> Result<()> {
    let mut data = acc.try_borrow_mut_data()?;
    let mut writer = &mut data[8..];
    tick.serialize(&mut writer)
        .map_err(|_| OrbitalError::TickSerializationFailed)?;
    Ok(())
}
