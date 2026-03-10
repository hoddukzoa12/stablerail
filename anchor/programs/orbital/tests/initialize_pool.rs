//! Integration test for initialize_pool instruction.
//!
//! Uses litesvm to simulate a real Solana runtime.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test initialize_pool -- --nocapture

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

/// Program ID matching declare_id! in lib.rs
const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w");

/// SPL Token program ID
const TOKEN_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// MAX_ASSETS from the program
const MAX_ASSETS: usize = 8;

/// Find the compiled .so file
fn program_so_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Navigate: programs/orbital → anchor → target/deploy
    path.pop(); // → programs
    path.pop(); // → anchor
    path.push("target/deploy/orbital.so");
    path
}

/// Create an SPL token mint via raw instructions
fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, decimals: u8) {
    let rent = svm.minimum_balance_for_rent_exemption(82);
    let create_ix = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        82,
        &TOKEN_PROGRAM_ID,
    );
    // spl_token InitializeMint2 instruction (type = 20)
    let mut init_data = vec![20]; // InitializeMint2
    init_data.push(decimals);
    init_data.extend_from_slice(payer.pubkey().as_ref()); // mint authority
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

/// Create an associated token account (ATA) and mint tokens to it
fn create_ata_and_mint(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    amount: u64,
) -> Pubkey {
    let ata = spl_associated_token_account_id(owner, mint);

    // Create ATA: use manual instruction since we don't have spl crate
    let create_ata_ix = Instruction {
        program_id: solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"),
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

    // MintTo instruction (type = 7)
    let mut mint_data = vec![7]; // MintTo
    mint_data.extend_from_slice(&amount.to_le_bytes());

    let mint_to_ix = Instruction {
        program_id: TOKEN_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*mint, false),
            AccountMeta::new(ata, false),
            AccountMeta::new_readonly(payer.pubkey(), true), // mint authority
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

/// Derive ATA address (matches spl-associated-token-account)
fn spl_associated_token_account_id(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
    let seeds = &[
        wallet.as_ref(),
        TOKEN_PROGRAM_ID.as_ref(),
        mint.as_ref(),
    ];
    let program = solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
    Pubkey::find_program_address(seeds, &program).0
}

/// Derive pool PDA
fn derive_pool_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"pool", authority.as_ref()], &PROGRAM_ID)
}

/// Derive vault PDA
fn derive_vault_pda(pool: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", pool.as_ref(), mint.as_ref()], &PROGRAM_ID)
}

/// Build InitPoolParams serialized data with Anchor discriminator
fn build_init_pool_data(
    n_assets: u8,
    fee_rate_bps: u16,
    initial_deposit_per_asset: u64,
    token_mints: [Pubkey; MAX_ASSETS],
) -> Vec<u8> {
    let discriminator = anchor_discriminator("global:initialize_pool");

    let mut data = Vec::new();
    data.extend_from_slice(&discriminator);
    data.push(n_assets);
    data.extend_from_slice(&fee_rate_bps.to_le_bytes());
    data.extend_from_slice(&initial_deposit_per_asset.to_le_bytes());
    for mint in &token_mints {
        data.extend_from_slice(mint.as_ref());
    }
    data
}

/// Compute Anchor instruction discriminator
fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = <sha2::Sha256 as sha2::Digest>::digest(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

#[test]
fn test_initialize_pool_creates_vaults_and_transfers() {
    // ── Setup ──
    let so_path = program_so_path();
    if !so_path.exists() {
        eprintln!(
            "Skipping integration test: program .so not found at {:?}. Run `cargo build-sbf -p orbital` first.",
            so_path
        );
        return;
    }

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(PROGRAM_ID, so_path.to_str().unwrap())
        .unwrap();
    // Also need SPL token and ATA programs
    // litesvm should have built-in SPL programs

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap(); // 10 SOL

    let n_assets: u8 = 3;
    let deposit: u64 = 1_000_000; // 1 USDC (6 decimals)

    // Create 3 mints
    let mut mints = Vec::new();
    for _ in 0..n_assets {
        let mint_kp = Keypair::new();
        create_mint(&mut svm, &authority, &mint_kp, 6);
        mints.push(mint_kp);
    }

    // Create ATAs and mint tokens
    let mut atas = Vec::new();
    for mint_kp in &mints {
        let ata = create_ata_and_mint(
            &mut svm,
            &authority,
            &mint_kp.pubkey(),
            &authority.pubkey(),
            deposit * 10, // mint extra for safety
        );
        atas.push(ata);
    }

    // ── Build instruction ──
    let (pool_pda, _pool_bump) = derive_pool_pda(&authority.pubkey());

    let mut token_mints_arr = [Pubkey::default(); MAX_ASSETS];
    for (i, mint_kp) in mints.iter().enumerate() {
        token_mints_arr[i] = mint_kp.pubkey();
    }

    let data = build_init_pool_data(n_assets, 30, deposit, token_mints_arr);

    // Accounts: authority, pool, system_program, token_program, rent
    let mut accounts = vec![
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];

    // remaining_accounts: mints, vaults, ATAs
    for mint_kp in &mints {
        accounts.push(AccountMeta::new_readonly(mint_kp.pubkey(), false));
    }
    let mut vault_pdas = Vec::new();
    for mint_kp in &mints {
        let (vault_pda, _bump) = derive_vault_pda(&pool_pda, &mint_kp.pubkey());
        accounts.push(AccountMeta::new(vault_pda, false));
        vault_pdas.push(vault_pda);
    }
    for ata in &atas {
        accounts.push(AccountMeta::new(*ata, false));
    }

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    };

    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "initialize_pool failed: {:?}",
        result.err()
    );

    // ── Verify pool state ──
    let pool_account = svm.get_account(&pool_pda).expect("pool account should exist");
    assert!(pool_account.data.len() > 0, "pool should have data");

    // ── Verify vault accounts ──
    for (i, vault_pda) in vault_pdas.iter().enumerate() {
        let vault_account = svm
            .get_account(vault_pda)
            .unwrap_or_else(|| panic!("vault {} should exist", i));
        assert_eq!(
            vault_account.owner, TOKEN_PROGRAM_ID,
            "vault {} should be owned by token program",
            i
        );
        assert_eq!(
            vault_account.data.len(),
            165, // TokenAccount::LEN
            "vault {} should have token account size",
            i
        );

        // Verify vault balance matches initial deposit
        // SPL token account layout: amount is at bytes [64..72] (little-endian u64)
        let amount = u64::from_le_bytes(
            vault_account.data[64..72]
                .try_into()
                .expect("amount slice"),
        );
        assert_eq!(
            amount, deposit,
            "vault {} balance should equal initial deposit",
            i
        );
    }
}
