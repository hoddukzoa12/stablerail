//! Shared test helpers for litesvm integration tests.
//!
//! Each test file includes this via `mod common;` and uses `common::*`.

use std::path::PathBuf;

use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};

// ── Constants ──

pub const PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w");
pub const TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ATA_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const MAX_ASSETS: usize = 8;

// ── Program Binary ──

pub fn program_so_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // → programs
    path.pop(); // → anchor
    path.push("target/deploy/orbital.so");
    path
}

// ── SPL Token Helpers ──

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, decimals: u8) {
    let rent = svm.minimum_balance_for_rent_exemption(82);
    let create_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        82,
        &TOKEN_PROGRAM_ID,
    );
    let mut init_data = vec![20]; // InitializeMint2
    init_data.push(decimals);
    init_data.extend_from_slice(payer.pubkey().as_ref());
    init_data.push(0); // no freeze authority

    let init_ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![AccountMeta::new(mint.pubkey(), false)],
        data: init_data,
    };

    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&payer.pubkey()),
        &[payer, mint],
        blockhash,
    );
    svm.send_transaction(tx).unwrap();
}

pub fn create_ata_and_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Pubkey {
    let ata = spl_associated_token_account_id(owner, mint);

    let create_ata_ix = Instruction {
        program_id: ATA_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: vec![],
    };

    let mut mint_data = vec![7]; // MintTo
    mint_data.extend_from_slice(&amount.to_le_bytes());

    let mint_to_ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        data: mint_data,
    };

    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[create_ata_ix, mint_to_ix],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    svm.send_transaction(tx).unwrap();

    ata
}

pub fn spl_associated_token_account_id(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            wallet.as_ref(),
            TOKEN_PROGRAM_ID.as_ref(),
            mint.as_ref(),
        ],
        &ATA_PROGRAM_ID,
    )
    .0
}

// ── Anchor Helpers ──

pub fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = <sha2::Sha256 as sha2::Digest>::digest(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

// ── PDA Derivation ──

pub fn derive_pool_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"pool", authority.as_ref()], &PROGRAM_ID)
}

pub fn derive_vault_pda(pool: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", pool.as_ref(), mint.as_ref()], &PROGRAM_ID)
}

// ── Instruction Data Builders ──

pub fn build_init_pool_data(
    n_assets: u8,
    fee_rate_bps: u16,
    initial_deposit: u64,
    token_mints: [Pubkey; MAX_ASSETS],
) -> Vec<u8> {
    let discriminator = anchor_discriminator("global:initialize_pool");

    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.push(n_assets);
    data.extend_from_slice(&fee_rate_bps.to_le_bytes());
    data.extend_from_slice(&initial_deposit.to_le_bytes());
    for mint in &token_mints {
        data.extend_from_slice(mint.as_ref());
    }
    data
}

// ── Additional SPL Token Helpers ──

/// Create ATA without minting (for a non-mint-authority owner)
pub fn create_ata(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, owner: &Pubkey) -> Pubkey {
    let ata = spl_associated_token_account_id(owner, mint);

    let create_ata_ix = Instruction {
        program_id: ATA_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(*owner, false),
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new_readonly(system_program::id(), false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        ],
        data: vec![],
    };

    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[create_ata_ix],
        Some(&payer.pubkey()),
        &[payer],
        blockhash,
    );
    svm.send_transaction(tx).unwrap();

    ata
}

/// Mint tokens to an existing ATA (requires mint_authority = payer)
pub fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, ata: &Pubkey, amount: u64) {
    let mut data = vec![7u8]; // MintTo
    data.extend_from_slice(&amount.to_le_bytes());

    let ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(*ata, false),
            AccountMeta::new_readonly(payer.pubkey(), true),
        ],
        data,
    };

    let blockhash = svm.latest_blockhash();
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    svm.send_transaction(tx).unwrap();
}

/// Read u64 token amount from SPL token account data at offset 64..72
pub fn read_token_amount(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let acc = svm
        .get_account(account)
        .unwrap_or_else(|| panic!("account {} should exist", account));
    u64::from_le_bytes(acc.data[64..72].try_into().expect("amount slice"))
}

// ── Error Extraction ──

pub fn extract_anchor_error_code(err: &str) -> Option<u32> {
    let start = err.find("Custom(")? + 7;
    let end = start + err[start..].find(')')?;
    err[start..end].parse().ok()
}

// ── Policy / Settlement Shared PDA Derivation ──

pub fn derive_policy_pda(pool: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"policy", pool.as_ref(), authority.as_ref()],
        &PROGRAM_ID,
    )
}

pub fn derive_allowlist_pda(policy: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"allowlist", policy.as_ref()], &PROGRAM_ID)
}

// ── Policy / Settlement Shared Instruction Data Builders ──

pub fn build_create_policy_data(max_trade_amount: u64, max_daily_volume: u64) -> Vec<u8> {
    let disc = anchor_discriminator("global:create_policy");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.extend_from_slice(&max_trade_amount.to_le_bytes());
    data.extend_from_slice(&max_daily_volume.to_le_bytes());
    data
}

pub fn build_update_policy_data(
    max_trade_amount: Option<u64>,
    max_daily_volume: Option<u64>,
    is_active: Option<bool>,
) -> Vec<u8> {
    let disc = anchor_discriminator("global:update_policy");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);

    match max_trade_amount {
        None => data.push(0),
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    match max_daily_volume {
        None => data.push(0),
        Some(v) => {
            data.push(1);
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    match is_active {
        None => data.push(0),
        Some(v) => {
            data.push(1);
            data.push(v as u8);
        }
    }
    data
}

pub fn build_manage_allowlist_data(action: u8, address: &Pubkey) -> Vec<u8> {
    let disc = anchor_discriminator("global:manage_allowlist");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.push(action); // 0 = Add, 1 = Remove
    data.extend_from_slice(address.as_ref());
    data
}

// ── Shared Math Helper ──

/// Compute the valid expected_amount_out using the orbital crate's analytical solver.
///
/// Uses the exact same Q64.64 math as the on-chain program to guarantee
/// the sphere invariant holds after the swap.
pub fn compute_valid_expected_out(
    n_assets: u8,
    deposit_per_asset: u64,
    fee_rate_bps: u16,
    token_in: usize,
    token_out: usize,
    amount_in: u64,
) -> u64 {
    use orbital::domain::core::{compute_fee, compute_radius_from_deposit};
    use orbital::math::newton::compute_amount_out_analytical;
    use orbital::math::{FixedPoint, Sphere};

    let per_asset = FixedPoint::checked_from_u64(deposit_per_asset).unwrap();
    let radius = compute_radius_from_deposit(per_asset, n_assets).unwrap();
    let sphere = Sphere {
        radius,
        n: n_assets,
    };
    let reserves: Vec<FixedPoint> = (0..n_assets as usize)
        .map(|_| per_asset)
        .collect();

    let amount_in_fp = FixedPoint::checked_from_u64(amount_in).unwrap();
    let fee = compute_fee(amount_in_fp, fee_rate_bps).unwrap();
    let net_in = amount_in_fp.checked_sub(fee).unwrap();
    let expected_out_fp =
        compute_amount_out_analytical(&sphere, &reserves, token_in, token_out, net_in).unwrap();
    expected_out_fp.to_u64().unwrap()
}
