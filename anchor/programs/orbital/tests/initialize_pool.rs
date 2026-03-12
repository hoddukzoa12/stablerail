//! Integration test for initialize_pool instruction.
//!
//! Uses litesvm to simulate a real Solana runtime.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test initialize_pool -- --nocapture

mod common;
use common::*;

use litesvm::LiteSVM;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};

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

    let data = build_init_pool_data(n_assets, 1, deposit, token_mints_arr);

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
    assert!(!pool_account.data.is_empty(), "pool should have data");

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
