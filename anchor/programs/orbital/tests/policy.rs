//! Integration tests for policy instructions (create_policy, update_policy, manage_allowlist).
//!
//! Uses litesvm to simulate a real Solana runtime.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test policy -- --nocapture

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

const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w");
const TOKEN_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const MAX_ASSETS: usize = 8;

// Q64.64 fractional bits for FixedPoint raw comparison
const FRAC_BITS: u32 = 64;

// ── Anchor error codes (6000 + OrbitalError variant index) ──
const ERROR_UNAUTHORIZED: u32 = 6021;
const ERROR_ALLOWLIST_FULL: u32 = 6024;
const ERROR_ALREADY_IN_ALLOWLIST: u32 = 6025;

// ── Shared Helpers (from swap.rs pattern) ──

fn program_so_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // → programs
    path.pop(); // → anchor
    path.push("target/deploy/orbital.so");
    path
}

fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint: &Keypair, decimals: u8) {
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

fn create_ata_and_mint(
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

fn spl_associated_token_account_id(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
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

fn anchor_discriminator(name: &str) -> [u8; 8] {
    let hash = <sha2::Sha256 as sha2::Digest>::digest(name.as_bytes());
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

fn derive_pool_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"pool", authority.as_ref()], &PROGRAM_ID)
}

fn derive_vault_pda(pool: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"vault", pool.as_ref(), mint.as_ref()], &PROGRAM_ID)
}

fn derive_policy_pda(pool: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"policy", pool.as_ref(), authority.as_ref()],
        &PROGRAM_ID,
    )
}

fn derive_allowlist_pda(policy: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"allowlist", policy.as_ref()], &PROGRAM_ID)
}

/// Convert u64 to Q64.64 raw i128 (matches FixedPoint::checked_from_u64)
fn u64_to_fp_raw(v: u64) -> i128 {
    (v as i128) << FRAC_BITS
}

/// Extract Anchor custom error code from litesvm error string.
fn extract_anchor_error_code(err: &str) -> Option<u32> {
    let start = err.find("Custom(")? + 7;
    let end = start + err[start..].find(')')?;
    err[start..end].parse().ok()
}

// ── Instruction Data Builders ──

fn build_init_pool_data(
    n_assets: u8,
    fee_rate_bps: u16,
    initial_deposit: u64,
    token_mints: [Pubkey; MAX_ASSETS],
) -> Vec<u8> {
    let disc = anchor_discriminator("global:initialize_pool");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.push(n_assets);
    data.extend_from_slice(&fee_rate_bps.to_le_bytes());
    data.extend_from_slice(&initial_deposit.to_le_bytes());
    for mint in &token_mints {
        data.extend_from_slice(mint.as_ref());
    }
    data
}

fn build_create_policy_data(max_trade_amount: u64, max_daily_volume: u64) -> Vec<u8> {
    let disc = anchor_discriminator("global:create_policy");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.extend_from_slice(&max_trade_amount.to_le_bytes());
    data.extend_from_slice(&max_daily_volume.to_le_bytes());
    data
}

fn build_update_policy_data(
    max_trade_amount: Option<u64>,
    max_daily_volume: Option<u64>,
    is_active: Option<bool>,
) -> Vec<u8> {
    let disc = anchor_discriminator("global:update_policy");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);

    // Borsh Option<u64>: 0 = None, 1 + le_bytes = Some
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

fn build_manage_allowlist_data(action: u8, address: &Pubkey) -> Vec<u8> {
    let disc = anchor_discriminator("global:manage_allowlist");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.push(action); // 0 = Add, 1 = Remove
    data.extend_from_slice(address.as_ref());
    data
}

// ── Account Data Readers ──

/// Read PolicyState fields from account data (after 8-byte Anchor discriminator).
/// Layout: bump(1) + authority(32) + pool(32) + max_trade(16) + max_daily(16)
///       + current_daily(16) + last_reset(8) + is_active(1) + created_at(8) + updated_at(8)
struct PolicyData {
    authority: Pubkey,
    pool: Pubkey,
    max_trade_amount_raw: i128,
    max_daily_volume_raw: i128,
    is_active: bool,
}

fn read_policy_data(svm: &LiteSVM, policy_pda: &Pubkey) -> PolicyData {
    let acc = svm
        .get_account(policy_pda)
        .unwrap_or_else(|| panic!("policy account {} should exist", policy_pda));
    let d = &acc.data[8..]; // skip discriminator

    let authority = Pubkey::try_from(&d[1..33]).unwrap();
    let pool = Pubkey::try_from(&d[33..65]).unwrap();
    let max_trade_amount_raw = i128::from_le_bytes(d[65..81].try_into().unwrap());
    let max_daily_volume_raw = i128::from_le_bytes(d[81..97].try_into().unwrap());
    // skip current_daily_volume(16) + last_reset_timestamp(8)
    let is_active = d[121] != 0;

    PolicyData {
        authority,
        pool,
        max_trade_amount_raw,
        max_daily_volume_raw,
        is_active,
    }
}

fn read_allowlist_count(svm: &LiteSVM, allowlist_pda: &Pubkey) -> u16 {
    let acc = svm
        .get_account(allowlist_pda)
        .unwrap_or_else(|| panic!("allowlist account {} should exist", allowlist_pda));
    let d = &acc.data[8..]; // skip discriminator
    // Layout: bump(1) + policy(32) + authority(32) + count(2)
    u16::from_le_bytes(d[65..67].try_into().unwrap())
}

// ── Test Scaffolding ──

struct TestEnv {
    svm: LiteSVM,
    authority: Keypair,
    pool_pda: Pubkey,
}

/// Initialize a minimal 2-asset pool (enough for policy tests).
fn setup_pool() -> TestEnv {
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

    let n_assets: u8 = 2;
    let deposit: u64 = 1_000_000;

    // Create 2 mints
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

    // remaining: mints, vaults, ATAs
    for mint_kp in &mints {
        accounts.push(AccountMeta::new_readonly(mint_kp.pubkey(), false));
    }
    for mint_kp in &mints {
        let (vault_pda, _) = derive_vault_pda(&pool_pda, &mint_kp.pubkey());
        accounts.push(AccountMeta::new(vault_pda, false));
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

    TestEnv {
        svm,
        authority,
        pool_pda,
    }
}

/// Send create_policy instruction. Returns Ok(policy_pda) or Err(error_string).
fn send_create_policy(
    env: &mut TestEnv,
    signer: &Keypair,
    max_trade_amount: u64,
    max_daily_volume: u64,
) -> Result<Pubkey, String> {
    let (policy_pda, _) = derive_policy_pda(&env.pool_pda, &signer.pubkey());

    let data = build_create_policy_data(max_trade_amount, max_daily_volume);

    let accounts = vec![
        AccountMeta::new(signer.pubkey(), true),
        AccountMeta::new_readonly(env.pool_pda, false),
        AccountMeta::new(policy_pda, false),
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
        Some(&signer.pubkey()),
        &[signer],
        blockhash,
    );

    env.svm
        .send_transaction(tx)
        .map(|_| policy_pda)
        .map_err(|e| format!("{:?}", e))
}

/// Send update_policy instruction.
fn send_update_policy(
    env: &mut TestEnv,
    signer: &Keypair,
    policy_pda: &Pubkey,
    max_trade_amount: Option<u64>,
    max_daily_volume: Option<u64>,
    is_active: Option<bool>,
) -> Result<(), String> {
    let data = build_update_policy_data(max_trade_amount, max_daily_volume, is_active);

    let accounts = vec![
        AccountMeta::new(signer.pubkey(), true),
        AccountMeta::new(*policy_pda, false),
    ];

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    };

    let blockhash = env.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        blockhash,
    );

    env.svm
        .send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Send manage_allowlist instruction (action: 0=Add, 1=Remove).
fn send_manage_allowlist(
    env: &mut TestEnv,
    signer: &Keypair,
    policy_pda: &Pubkey,
    action: u8,
    address: &Pubkey,
) -> Result<Pubkey, String> {
    let (allowlist_pda, _) = derive_allowlist_pda(policy_pda);

    let data = build_manage_allowlist_data(action, address);

    let accounts = vec![
        AccountMeta::new(signer.pubkey(), true),
        AccountMeta::new_readonly(*policy_pda, false),
        AccountMeta::new(allowlist_pda, false),
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
        Some(&signer.pubkey()),
        &[signer],
        blockhash,
    );

    env.svm
        .send_transaction(tx)
        .map(|_| allowlist_pda)
        .map_err(|e| format!("{:?}", e))
}

// ══════════════════════════════════════════════
// Test 1: create_policy succeeds and stores correct fields
// ══════════════════════════════════════════════

#[test]
fn test_create_policy() {
    let mut env = setup_pool();
    let auth = env.authority.insecure_clone();

    let max_trade: u64 = 1_000_000;
    let max_daily: u64 = 10_000_000;

    let policy_pda = send_create_policy(&mut env, &auth, max_trade, max_daily)
        .expect("create_policy should succeed");

    let policy = read_policy_data(&env.svm, &policy_pda);

    assert_eq!(policy.authority, auth.pubkey(), "authority mismatch");
    assert_eq!(policy.pool, env.pool_pda, "pool mismatch");
    assert_eq!(
        policy.max_trade_amount_raw,
        u64_to_fp_raw(max_trade),
        "max_trade_amount mismatch"
    );
    assert_eq!(
        policy.max_daily_volume_raw,
        u64_to_fp_raw(max_daily),
        "max_daily_volume mismatch"
    );
    assert!(policy.is_active, "policy should be active by default");
}

// ══════════════════════════════════════════════
// Test 2: update_policy changes fields correctly
// ══════════════════════════════════════════════

#[test]
fn test_update_policy() {
    let mut env = setup_pool();
    let auth = env.authority.insecure_clone();

    let policy_pda = send_create_policy(&mut env, &auth, 1_000_000, 10_000_000)
        .expect("create_policy should succeed");

    // Update max_trade_amount and is_active, leave max_daily_volume unchanged
    let new_max_trade: u64 = 2_000_000;
    send_update_policy(
        &mut env,
        &auth,
        &policy_pda,
        Some(new_max_trade),
        None,
        Some(false),
    )
    .expect("update_policy should succeed");

    let policy = read_policy_data(&env.svm, &policy_pda);
    assert_eq!(
        policy.max_trade_amount_raw,
        u64_to_fp_raw(new_max_trade),
        "max_trade_amount should be updated"
    );
    assert_eq!(
        policy.max_daily_volume_raw,
        u64_to_fp_raw(10_000_000),
        "max_daily_volume should be unchanged"
    );
    assert!(!policy.is_active, "policy should be deactivated");
}

// ══════════════════════════════════════════════
// Test 3: manage_allowlist add and remove
// ══════════════════════════════════════════════

#[test]
fn test_manage_allowlist_add_remove() {
    let mut env = setup_pool();
    let auth = env.authority.insecure_clone();

    let policy_pda = send_create_policy(&mut env, &auth, 1_000_000, 10_000_000)
        .expect("create_policy should succeed");

    let member = Pubkey::new_unique();

    // Add member
    let allowlist_pda = send_manage_allowlist(&mut env, &auth, &policy_pda, 0, &member)
        .expect("add member should succeed");

    let count = read_allowlist_count(&env.svm, &allowlist_pda);
    assert_eq!(count, 1, "count should be 1 after add");

    // Remove member
    send_manage_allowlist(&mut env, &auth, &policy_pda, 1, &member)
        .expect("remove member should succeed");

    let count = read_allowlist_count(&env.svm, &allowlist_pda);
    assert_eq!(count, 0, "count should be 0 after remove");
}

// ══════════════════════════════════════════════
// Test 4: create_policy rejects non-authority
// ══════════════════════════════════════════════

#[test]
fn test_create_policy_rejects_non_authority() {
    let mut env = setup_pool();

    // Create a different signer who is NOT the pool authority
    let impostor = Keypair::new();
    env.svm
        .airdrop(&impostor.pubkey(), 5_000_000_000)
        .unwrap();

    let result = send_create_policy(&mut env, &impostor, 1_000_000, 10_000_000);
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_UNAUTHORIZED),
        "expected Unauthorized (6021), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 5: allowlist rejects duplicate address
// ══════════════════════════════════════════════

#[test]
fn test_allowlist_rejects_duplicate() {
    let mut env = setup_pool();
    let auth = env.authority.insecure_clone();

    let policy_pda = send_create_policy(&mut env, &auth, 1_000_000, 10_000_000)
        .expect("create_policy should succeed");

    let member = Pubkey::new_unique();

    // First add succeeds
    send_manage_allowlist(&mut env, &auth, &policy_pda, 0, &member)
        .expect("first add should succeed");

    // Expire blockhash so litesvm doesn't reject as duplicate transaction
    env.svm.expire_blockhash();

    // Second add with same address should fail
    let result = send_manage_allowlist(&mut env, &auth, &policy_pda, 0, &member);
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_ALREADY_IN_ALLOWLIST),
        "expected AlreadyInAllowlist (6025), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 6: allowlist rejects when full (20 members)
// ══════════════════════════════════════════════

#[test]
fn test_allowlist_rejects_full() {
    let mut env = setup_pool();
    let auth = env.authority.insecure_clone();

    let policy_pda = send_create_policy(&mut env, &auth, 1_000_000, 10_000_000)
        .expect("create_policy should succeed");

    // Add 20 unique members (MAX_ALLOWLIST_SIZE)
    for i in 0..20u8 {
        let mut bytes = [0u8; 32];
        bytes[0] = i + 1; // unique non-zero addresses
        let member = Pubkey::from(bytes);
        send_manage_allowlist(&mut env, &auth, &policy_pda, 0, &member)
            .unwrap_or_else(|e| panic!("add member {} should succeed: {}", i, e));
    }

    let (allowlist_pda, _) = derive_allowlist_pda(&policy_pda);
    let count = read_allowlist_count(&env.svm, &allowlist_pda);
    assert_eq!(count, 20, "count should be 20 at capacity");

    // 21st member should fail
    let overflow_member = Pubkey::new_unique();
    let result = send_manage_allowlist(&mut env, &auth, &policy_pda, 0, &overflow_member);
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_ALLOWLIST_FULL),
        "expected AllowlistFull (6024), got: {err}"
    );
}
