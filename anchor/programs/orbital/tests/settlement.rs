//! Integration tests for execute_settlement instruction.
//!
//! Settlement wraps execute_swap with policy enforcement (allowlist, trade limits,
//! daily volume) and creates immutable audit entries.
//!
//! Uses litesvm to simulate a real Solana runtime.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test settlement -- --nocapture

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

// ── Anchor error codes (6000 + OrbitalError variant index) ──
const ERROR_UNAUTHORIZED: u32 = 6021;
const ERROR_POLICY_LIMIT_EXCEEDED: u32 = 6023;
const ERROR_SETTLEMENT_POLICY_VIOLATION: u32 = 6027;
const ERROR_DAILY_VOLUME_LIMIT_EXCEEDED: u32 = 6037;

// ── Settlement-Specific PDA Derivation ──

fn derive_settlement_pda(pool: &Pubkey, executor: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"settlement",
            pool.as_ref(),
            executor.as_ref(),
            &nonce.to_le_bytes(),
        ],
        &PROGRAM_ID,
    )
}

fn derive_audit_pda(settlement: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"audit", settlement.as_ref()], &PROGRAM_ID)
}

// ── Instruction Data Builders (settlement-specific) ──

fn build_execute_settlement_data(
    token_in_index: u8,
    token_out_index: u8,
    amount: u64,
    min_amount_out: u64,
    nonce: u64,
) -> Vec<u8> {
    let disc = anchor_discriminator("global:execute_settlement");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.push(token_in_index);
    data.push(token_out_index);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&min_amount_out.to_le_bytes());
    data.extend_from_slice(&nonce.to_le_bytes());
    data
}

// ── Test Scaffolding ──

struct SettlementTestEnv {
    svm: LiteSVM,
    authority: Keypair,
    pool_pda: Pubkey,
    mints: Vec<Keypair>,
    vault_pdas: Vec<Pubkey>,
    policy_pda: Pubkey,
    allowlist_pda: Pubkey,
    n_assets: u8,
    deposit: u64,
}

/// Initialize a 3-asset pool + policy + allowlist for settlement tests.
fn setup_settlement_env(deposit: u64, max_trade: u64, max_daily: u64) -> SettlementTestEnv {
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

    // Create ATAs and mint tokens
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

    // Create policy
    let (policy_pda, _) = derive_policy_pda(&pool_pda, &authority.pubkey());
    let policy_data = build_create_policy_data(max_trade, max_daily);
    let policy_accounts = vec![
        AccountMeta::new(authority.pubkey(), true),
        AccountMeta::new_readonly(pool_pda, false),
        AccountMeta::new(policy_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    let policy_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: policy_accounts,
        data: policy_data,
    };
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[policy_ix],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    svm.send_transaction(tx)
        .expect("create_policy should succeed");

    // Initialize allowlist by adding authority as first member
    let (allowlist_pda, _) = derive_allowlist_pda(&policy_pda);
    {
        let al_data = build_manage_allowlist_data(0, &authority.pubkey());
        let al_accounts = vec![
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(policy_pda, false),
            AccountMeta::new(allowlist_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ];
        let al_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: al_accounts,
            data: al_data,
        };
        let blockhash = svm.latest_blockhash();
        let tx = Transaction::new_signed_with_payer(
            &[al_ix],
            Some(&authority.pubkey()),
            &[&authority],
            blockhash,
        );
        svm.send_transaction(tx)
            .expect("init allowlist should succeed");
    }

    SettlementTestEnv {
        svm,
        authority,
        pool_pda,
        mints,
        vault_pdas,
        policy_pda,
        allowlist_pda,
        n_assets,
        deposit,
    }
}

/// Add an executor to the allowlist.
fn add_to_allowlist(env: &mut SettlementTestEnv, member: &Pubkey) {
    let data = build_manage_allowlist_data(0, member);
    let accounts = vec![
        AccountMeta::new(env.authority.pubkey(), true),
        AccountMeta::new_readonly(env.policy_pda, false),
        AccountMeta::new(env.allowlist_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    };
    let blockhash = env.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&env.authority.pubkey()),
        &[&env.authority],
        blockhash,
    );
    env.svm
        .send_transaction(tx)
        .expect("add_to_allowlist should succeed");
}

/// Create a funded executor with ATAs for all pool tokens (not yet on the allowlist).
fn create_funded_executor(env: &mut SettlementTestEnv, fund_amount: u64) -> (Keypair, Vec<Pubkey>) {
    let executor = Keypair::new();
    env.svm
        .airdrop(&executor.pubkey(), 5_000_000_000)
        .unwrap();

    let mut executor_atas = Vec::new();
    for mint_kp in &env.mints {
        let ata = create_ata(&mut env.svm, &env.authority, &mint_kp.pubkey(), &executor.pubkey());
        mint_to(
            &mut env.svm,
            &env.authority,
            &mint_kp.pubkey(),
            &ata,
            fund_amount,
        );
        executor_atas.push(ata);
    }

    (executor, executor_atas)
}

/// Create an executor with funded ATAs and add to allowlist.
fn setup_executor(env: &mut SettlementTestEnv, fund_amount: u64) -> (Keypair, Vec<Pubkey>) {
    let (executor, atas) = create_funded_executor(env, fund_amount);
    add_to_allowlist(env, &executor.pubkey());
    (executor, atas)
}

/// Send execute_settlement instruction. Returns Ok(settlement_pda) or Err.
fn send_settlement(
    env: &mut SettlementTestEnv,
    executor: &Keypair,
    executor_atas: &[Pubkey],
    token_in: usize,
    token_out: usize,
    amount: u64,
    min_amount_out: u64,
    nonce: u64,
) -> Result<Pubkey, String> {
    let (settlement_pda, _) = derive_settlement_pda(&env.pool_pda, &executor.pubkey(), nonce);
    let (audit_pda, _) = derive_audit_pda(&settlement_pda);

    let data = build_execute_settlement_data(
        token_in as u8,
        token_out as u8,
        amount,
        min_amount_out,
        nonce,
    );

    let accounts = vec![
        AccountMeta::new(executor.pubkey(), true),
        AccountMeta::new(env.pool_pda, false),
        AccountMeta::new(env.policy_pda, false),
        AccountMeta::new_readonly(env.allowlist_pda, false),
        AccountMeta::new(settlement_pda, false),
        AccountMeta::new(audit_pda, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        AccountMeta::new_readonly(system_program::id(), false),
        // remaining_accounts: vault_in, vault_out, executor_ata_in, executor_ata_out
        AccountMeta::new(env.vault_pdas[token_in], false),
        AccountMeta::new(env.vault_pdas[token_out], false),
        AccountMeta::new(executor_atas[token_in], false),
        AccountMeta::new(executor_atas[token_out], false),
    ];

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    };

    let blockhash = env.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&executor.pubkey()),
        &[executor],
        blockhash,
    );

    env.svm
        .send_transaction(tx)
        .map(|_| settlement_pda)
        .map_err(|e| format!("{:?}", e))
}

// ── Account Data Readers ──

/// Read SettlementState status field (0=Pending, 1=Executed, 2=Failed).
fn read_settlement_status(svm: &LiteSVM, settlement_pda: &Pubkey) -> u8 {
    let acc = svm
        .get_account(settlement_pda)
        .unwrap_or_else(|| panic!("settlement account {} should exist", settlement_pda));
    let d = &acc.data[8..]; // skip discriminator
    // Layout: bump(1) + pool(32) + policy(32) + executor(32)
    //       + token_in_index(1) + token_out_index(1)
    //       + amount_in(16) + amount_out(16) + execution_price(16) + status(1)
    let offset = 1 + 32 + 32 + 32 + 1 + 1 + 16 + 16 + 16;
    d[offset]
}

/// Read AuditEntryState action_hash field.
fn read_audit_action_hash(svm: &LiteSVM, audit_pda: &Pubkey) -> [u8; 32] {
    let acc = svm
        .get_account(audit_pda)
        .unwrap_or_else(|| panic!("audit account {} should exist", audit_pda));
    let d = &acc.data[8..]; // skip discriminator
    // Layout: bump(1) + settlement(32) + executor(32) + pool(32) + policy(32) + action_hash(32)
    let offset = 1 + 32 + 32 + 32 + 32;
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&d[offset..offset + 32]);
    hash
}

// ══════════════════════════════════════════════
// Test 1: settlement executes swap correctly
// ══════════════════════════════════════════════

#[test]
fn test_settlement_executes_swap() {
    let mut env = setup_settlement_env(1_000_000, 500_000, 10_000_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    let amount: u64 = 10_000;
    let expected_out = compute_valid_expected_out(
        env.n_assets, env.deposit, 30, 0, 1, amount, 6,
    );
    assert!(expected_out > 0, "expected_out must be positive");

    // Record pre-settlement balances
    let vault_in_before = read_token_amount(&env.svm, &env.vault_pdas[0]);
    let vault_out_before = read_token_amount(&env.svm, &env.vault_pdas[1]);
    let exec_ata_in_before = read_token_amount(&env.svm, &executor_atas[0]);
    let exec_ata_out_before = read_token_amount(&env.svm, &executor_atas[1]);

    let settlement_pda = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, amount, 1, 0,
    )
    .expect("settlement should succeed");

    // Verify vault balances
    let vault_in_after = read_token_amount(&env.svm, &env.vault_pdas[0]);
    let vault_out_after = read_token_amount(&env.svm, &env.vault_pdas[1]);
    assert_eq!(
        vault_in_after,
        vault_in_before + amount,
        "vault_in should receive amount"
    );
    assert_eq!(
        vault_out_after,
        vault_out_before - expected_out,
        "vault_out should send expected_out"
    );

    // Verify executor ATA balances
    let exec_ata_in_after = read_token_amount(&env.svm, &executor_atas[0]);
    let exec_ata_out_after = read_token_amount(&env.svm, &executor_atas[1]);
    assert_eq!(
        exec_ata_in_after,
        exec_ata_in_before - amount,
        "executor should spend amount"
    );
    assert_eq!(
        exec_ata_out_after,
        exec_ata_out_before + expected_out,
        "executor should receive expected_out"
    );

    // Verify settlement status = Executed (1)
    let status = read_settlement_status(&env.svm, &settlement_pda);
    assert_eq!(status, 1, "settlement status should be Executed (1)");
}

// ══════════════════════════════════════════════
// Test 2: settlement rejects unauthorized executor
// ══════════════════════════════════════════════

#[test]
fn test_settlement_rejects_unauthorized() {
    let mut env = setup_settlement_env(1_000_000, 500_000, 10_000_000);

    // Create executor but do NOT add to allowlist
    let (executor, executor_atas) = create_funded_executor(&mut env, 100_000);

    let result = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, 0,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_UNAUTHORIZED),
        "expected Unauthorized (6021), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 3: settlement rejects amount exceeding trade limit
// ══════════════════════════════════════════════

#[test]
fn test_settlement_rejects_exceeds_trade_limit() {
    // Set max_trade_amount to 5000 (very low)
    let mut env = setup_settlement_env(1_000_000, 5_000, 10_000_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    // Try to settle 10_000 which exceeds max_trade_amount of 5_000
    let result = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, 0,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_POLICY_LIMIT_EXCEEDED),
        "expected PolicyLimitExceeded (6023), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 4: settlement creates audit entry with valid action_hash
// ══════════════════════════════════════════════

#[test]
fn test_settlement_creates_audit_entry() {
    let mut env = setup_settlement_env(1_000_000, 500_000, 10_000_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    let nonce: u64 = 42;
    let settlement_pda = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, nonce,
    )
    .expect("settlement should succeed");

    // Verify audit entry exists and has non-zero action_hash
    // (read_audit_action_hash panics if the account is absent)
    let (audit_pda, _) = derive_audit_pda(&settlement_pda);
    let action_hash = read_audit_action_hash(&env.svm, &audit_pda);
    assert_ne!(
        action_hash,
        [0u8; 32],
        "action_hash should not be all zeros"
    );
}

// ══════════════════════════════════════════════
// Test 5: settlement rejects when policy is inactive
// ══════════════════════════════════════════════

#[test]
fn test_settlement_rejects_inactive_policy() {
    let mut env = setup_settlement_env(1_000_000, 500_000, 10_000_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    // Deactivate policy via update_policy(is_active = false)
    let update_data = build_update_policy_data(None, None, Some(false));
    let update_accounts = vec![
        AccountMeta::new(env.authority.pubkey(), true),
        AccountMeta::new(env.policy_pda, false),
    ];
    let update_ix = Instruction {
        program_id: PROGRAM_ID,
        accounts: update_accounts,
        data: update_data,
    };
    let blockhash = env.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[update_ix],
        Some(&env.authority.pubkey()),
        &[&env.authority],
        blockhash,
    );
    env.svm
        .send_transaction(tx)
        .expect("update_policy should succeed");

    // Now attempt settlement — should fail because policy.is_active = false
    let result = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, 0,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_SETTLEMENT_POLICY_VIOLATION),
        "expected SettlementPolicyViolation (6027), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 6: settlement rejects when daily volume exceeded
// ══════════════════════════════════════════════

#[test]
fn test_settlement_rejects_daily_volume_exceeded() {
    // Set max_daily_volume to 15_000 so two 10_000 settlements exceed it
    let mut env = setup_settlement_env(1_000_000, 500_000, 15_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    // First settlement (10_000) should succeed — cumulative = 10_000 <= 15_000
    send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, 0,
    )
    .expect("first settlement should succeed");

    // Second settlement (10_000) should fail — cumulative = 20_000 > 15_000
    let result = send_settlement(
        &mut env, &executor, &executor_atas, 0, 1, 10_000, 1, 1,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_DAILY_VOLUME_LIMIT_EXCEEDED),
        "expected DailyVolumeLimitExceeded (6037), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 7: settlement rejects foreign-owned ATA out
// ══════════════════════════════════════════════

#[test]
fn test_settlement_rejects_foreign_ata_out() {
    let mut env = setup_settlement_env(1_000_000, 500_000, 10_000_000);
    let (executor, executor_atas) = setup_executor(&mut env, 500_000);

    // Create a foreign user with an ATA for token_out mint (index 1)
    let foreign = Keypair::new();
    env.svm
        .airdrop(&foreign.pubkey(), 1_000_000_000)
        .unwrap();
    let foreign_ata_out = create_ata(
        &mut env.svm,
        &env.authority,
        &env.mints[1].pubkey(),
        &foreign.pubkey(),
    );

    // Replace executor_ata_out with foreign-owned ATA
    let mut spoofed_atas = executor_atas.clone();
    spoofed_atas[1] = foreign_ata_out;

    let result = send_settlement(
        &mut env, &executor, &spoofed_atas, 0, 1, 10_000, 1, 0,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_UNAUTHORIZED),
        "expected Unauthorized (6021) for foreign-owned ATA out, got: {err}"
    );
}
