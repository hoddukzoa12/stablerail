use anchor_lang::prelude::*;

pub const MAX_ALLOWLIST_SIZE: usize = 50;

#[account]
pub struct AllowlistState {
    pub bump: u8,
    pub policy: Pubkey,
    pub authority: Pubkey,
    pub count: u16,
    pub addresses: [Pubkey; MAX_ALLOWLIST_SIZE],
    pub _reserved: [u8; 64],
}

impl AllowlistState {
    pub const SIZE: usize = 8 + 1 + 32 + 32 + 2 + (32 * MAX_ALLOWLIST_SIZE) + 64;

    pub fn contains(&self, address: &Pubkey) -> bool {
        self.addresses[..self.count as usize]
            .iter()
            .any(|a| a == address)
    }

    pub fn add(&mut self, address: Pubkey) -> Result<()> {
        require!(
            (self.count as usize) < MAX_ALLOWLIST_SIZE,
            crate::errors::OrbitalError::AllowlistFull
        );
        require!(
            !self.contains(&address),
            crate::errors::OrbitalError::AlreadyInAllowlist
        );
        self.addresses[self.count as usize] = address;
        self.count += 1;
        Ok(())
    }

    pub fn remove(&mut self, address: &Pubkey) -> Result<()> {
        let pos = self.addresses[..self.count as usize]
            .iter()
            .position(|a| a == address)
            .ok_or(crate::errors::OrbitalError::NotInAllowlist)?;
        let last = self.count as usize - 1;
        self.addresses[pos] = self.addresses[last];
        self.addresses[last] = Pubkey::default();
        self.count -= 1;
        Ok(())
    }
}
