//! Integration tests for add_liquidity and remove_liquidity instructions.
//!
//! Uses litesvm to simulate a real Solana runtime.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test liquidity -- --nocapture

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

// ── Liquidity-Specific PDA Derivation ──

fn derive_position_pda(pool: &Pubkey, owner: &Pubkey, position_count: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"position",
            pool.as_ref(),
            owner.as_ref(),
            &position_count.to_le_bytes(),
        ],
        &PROGRAM_ID,
    )
}

// ── Instruction Builders ──

fn build_add_liquidity_data(amounts: [u64; MAX_ASSETS]) -> Vec<u8> {
    let disc = anchor_discriminator("global:add_liquidity");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    for amount in &amounts {
        data.extend_from_slice(&amount.to_le_bytes());
    }
    data
}

fn build_remove_liquidity_data(liquidity_amount: u64) -> Vec<u8> {
    let disc = anchor_discriminator("global:remove_liquidity");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.extend_from_slice(&liquidity_amount.to_le_bytes());
    data
}

// ── Test Scaffolding ──

struct TestPool {
    svm: LiteSVM,
    authority: Keypair,
    pool_pda: Pubkey,
    mints: Vec<Keypair>,
    vault_pdas: Vec<Pubkey>,
    authority_atas: Vec<Pubkey>,
    n_assets: u8,
    deposit: u64,
}

/// Initialize a 3-asset pool with the given deposit per asset.
fn setup_pool(deposit: u64) -> TestPool {
    let so_path = program_so_path();
    if !so_path.exists() {
        panic!(
            "Program .so not found at {:?}. Run `cargo build-sbf -p orbital` first.",
            so_path
        );
    }

    let mut svm = LiteSVM::new();
    svm.add_program_from_file(PROGRAM_ID, so_path.to_str().unwrap())
        .unwrap();

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let n_assets: u8 = 3;

    // Create 3 mints
    let mut mints = Vec::new();
    for _ in 0..n_assets {
        let mint_kp = Keypair::new();
        create_mint(&mut svm, &authority, &mint_kp, 6);
        mints.push(mint_kp);
    }

    // Create ATAs and mint tokens (extra for subsequent operations)
    let mut authority_atas = Vec::new();
    for mint_kp in &mints {
        let ata = create_ata_and_mint(
            &mut svm,
            &authority,
            &mint_kp.pubkey(),
            &authority.pubkey(),
            deposit * 10,
        );
        authority_atas.push(ata);
    }

    // Initialize pool
    let (pool_pda, _) = derive_pool_pda(&authority.pubkey());

    let mut token_mints_arr = [Pubkey::default(); MAX_ASSETS];
    for (i, mint_kp) in mints.iter().enumerate() {
        token_mints_arr[i] = mint_kp.pubkey();
    }

    let data = build_init_pool_data(n_assets, 30, deposit, token_mints_arr);

    let mut accounts = vec![
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new(pool_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(solana_sdk::sysvar::rent::id(), false),
    ];

    // remaining: mints, vaults, ATAs
    for mint_kp in &mints {
        accounts.push(AccountMeta::new_readonly(mint_kp.pubkey(), false));
    }
    let mut vault_pdas = Vec::new();
    for mint_kp in &mints {
        let (vault_pda, _) = derive_vault_pda(&pool_pda, &mint_kp.pubkey());
        accounts.push(AccountMeta::new(vault_pda, false));
        vault_pdas.push(vault_pda);
    }
    for ata in &authority_atas {
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
    svm.send_transaction(tx)
        .expect("initialize_pool should succeed");

    TestPool {
        svm,
        authority,
        pool_pda,
        mints,
        vault_pdas,
        authority_atas,
        n_assets,
        deposit,
    }
}

// ── Provider helper: create ATAs, mint tokens, execute add_liquidity ──

struct ProviderSetup {
    provider: Keypair,
    provider_atas: Vec<Pubkey>,
    position_pda: Pubkey,
}

/// Create a provider with funded ATAs and execute add_liquidity.
fn add_provider_liquidity(tp: &mut TestPool, add_amount: u64, position_index: u64) -> ProviderSetup {
    let provider = Keypair::new();
    tp.svm
        .airdrop(&provider.pubkey(), 5_000_000_000)
        .unwrap();

    let mut provider_atas = Vec::new();
    for mint_kp in &tp.mints {
        let ata = create_ata(&mut tp.svm, &tp.authority, &mint_kp.pubkey(), &provider.pubkey());
        mint_to(
            &mut tp.svm,
            &tp.authority,
            &mint_kp.pubkey(),
            &ata,
            add_amount,
        );
        provider_atas.push(ata);
    }

    let mut amounts = [0u64; MAX_ASSETS];
    for i in 0..(tp.n_assets as usize) {
        amounts[i] = add_amount;
    }

    let (position_pda, _) = derive_position_pda(&tp.pool_pda, &provider.pubkey(), position_index);

    let mut accounts = vec![
        AccountMeta::new(provider.pubkey(), true),
        AccountMeta::new(tp.pool_pda, false),
        AccountMeta::new(position_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ];
    for vault in &tp.vault_pdas {
        accounts.push(AccountMeta::new(*vault, false));
    }
    for ata in &provider_atas {
        accounts.push(AccountMeta::new(*ata, false));
    }

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data: build_add_liquidity_data(amounts),
    };

    let blockhash = tp.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&provider.pubkey()),
        &[&provider],
        blockhash,
    );
    tp.svm
        .send_transaction(tx)
        .expect("add_liquidity should succeed");

    ProviderSetup {
        provider,
        provider_atas,
        position_pda,
    }
}

// ══════════════════════════════════════════════
// Test 1: add_liquidity deposits tokens and creates position
// ══════════════════════════════════════════════

#[test]
fn test_add_liquidity_deposits_and_creates_position() {
    let mut tp = setup_pool(1_000_000);
    let add_amount: u64 = 500_000;

    let ps = add_provider_liquidity(&mut tp, add_amount, 0);

    // ── Verify vault balances ──
    for (i, vault) in tp.vault_pdas.iter().enumerate() {
        let balance = read_token_amount(&tp.svm, vault);
        let expected = tp.deposit + add_amount; // 1_000_000 + 500_000
        assert_eq!(
            balance, expected,
            "vault {} balance should be {}, got {}",
            i, expected, balance
        );
    }

    // ── Verify position account exists ──
    let position_account = tp
        .svm
        .get_account(&ps.position_pda)
        .expect("position account should exist");
    assert!(
        !position_account.data.is_empty(),
        "position should have data"
    );

    // ── Verify provider ATAs are drained ──
    for (i, ata) in ps.provider_atas.iter().enumerate() {
        let balance = read_token_amount(&tp.svm, ata);
        assert_eq!(
            balance, 0,
            "provider ATA {} should be empty after deposit, got {}",
            i, balance
        );
    }
}

// ══════════════════════════════════════════════
// Test 2: remove_liquidity returns tokens proportionally
// ══════════════════════════════════════════════

#[test]
fn test_remove_liquidity_returns_tokens() {
    let mut tp = setup_pool(1_000_000);
    let add_amount: u64 = 500_000;

    let ps = add_provider_liquidity(&mut tp, add_amount, 0);

    // Now remove all liquidity
    // liquidity = sum of deposits = 500_000 * 3 = 1_500_000
    let liquidity_to_remove: u64 = add_amount * (tp.n_assets as u64);
    let remove_data = build_remove_liquidity_data(liquidity_to_remove);

    let mut remove_accounts = vec![
        AccountMeta::new(ps.provider.pubkey(), true),
        AccountMeta::new(tp.pool_pda, false),
        AccountMeta::new(ps.position_pda, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ];
    for vault in &tp.vault_pdas {
        remove_accounts.push(AccountMeta::new(*vault, false));
    }
    for ata in &ps.provider_atas {
        remove_accounts.push(AccountMeta::new(*ata, false));
    }

    let remove_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: remove_accounts,
        data: remove_data,
    };

    let blockhash = tp.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[remove_ix],
        Some(&ps.provider.pubkey()),
        &[&ps.provider],
        blockhash,
    );

    let result = tp.svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "remove_liquidity failed: {:?}",
        result.err()
    );

    // ── Verify provider got tokens back ──
    for (i, ata) in ps.provider_atas.iter().enumerate() {
        let balance = read_token_amount(&tp.svm, ata);
        assert!(
            balance > 0,
            "provider ATA {} should have tokens after remove, got 0",
            i
        );
    }

    // ── Verify vault balances decreased ──
    for (i, vault) in tp.vault_pdas.iter().enumerate() {
        let balance = read_token_amount(&tp.svm, vault);
        assert!(
            balance < tp.deposit + add_amount,
            "vault {} should have less than initial+add, got {}",
            i,
            balance
        );
    }
}

// ══════════════════════════════════════════════
// Test 3: add_liquidity rejects zero amount
// ══════════════════════════════════════════════

#[test]
fn test_add_liquidity_rejects_zero_amount() {
    let mut tp = setup_pool(1_000_000);

    let provider = Keypair::new();
    tp.svm
        .airdrop(&provider.pubkey(), 5_000_000_000)
        .unwrap();

    let mut provider_atas = Vec::new();
    for mint_kp in &tp.mints {
        let ata = create_ata(&mut tp.svm, &tp.authority, &mint_kp.pubkey(), &provider.pubkey());
        mint_to(&mut tp.svm, &tp.authority, &mint_kp.pubkey(), &ata, 1_000_000);
        provider_atas.push(ata);
    }

    // amounts with a zero entry — should cause rejection
    let mut amounts = [0u64; MAX_ASSETS];
    amounts[0] = 500_000;
    amounts[1] = 0;
    amounts[2] = 500_000;

    let (position_pda, _) = derive_position_pda(&tp.pool_pda, &provider.pubkey(), 0);

    let mut accounts = vec![
        AccountMeta::new(provider.pubkey(), true),
        AccountMeta::new(tp.pool_pda, false),
        AccountMeta::new(position_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
    ];
    for vault in &tp.vault_pdas {
        accounts.push(AccountMeta::new(*vault, false));
    }
    for ata in &provider_atas {
        accounts.push(AccountMeta::new(*ata, false));
    }

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data: build_add_liquidity_data(amounts),
    };

    let blockhash = tp.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&provider.pubkey()),
        &[&provider],
        blockhash,
    );

    assert!(
        tp.svm.send_transaction(tx).is_err(),
        "add_liquidity should reject zero amount"
    );
}
