//! Integration tests for execute_swap instruction with SPL token CPI.
//!
//! Uses litesvm to simulate a real Solana runtime.
//! Uses orbital crate's Q64.64 math for exact expected_amount_out computation.
//!
//! Prerequisites:
//!   cargo build-sbf -p orbital
//!
//! Run:
//!   cargo test --test swap -- --nocapture

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

// Import orbital crate for chained swap math (compute_valid_expected_out is in common)
use orbital::domain::core::{compute_fee, compute_radius_from_deposit};
use orbital::math::newton::compute_amount_out_analytical;
use orbital::math::{FixedPoint, Sphere};

// ── Anchor error codes (6000 + OrbitalError variant index) ──
const ERROR_SLIPPAGE_EXCEEDED: u32 = 6010;
const ERROR_SAME_TOKEN_SWAP: u32 = 6011;

// ── Swap-Specific Helpers ──

/// Compute expected_amount_out for a swap on a pool with UPDATED reserves.
///
/// Used for chained swaps where reserves are no longer equal.
fn compute_expected_out_with_reserves(
    n_assets: u8,
    deposit_per_asset: u64,
    fee_rate_bps: u16,
    reserves_u64: &[u64],
    token_in: usize,
    token_out: usize,
    amount_in: u64,
) -> u64 {
    let per_asset = FixedPoint::checked_from_u64(deposit_per_asset).unwrap();
    let radius = compute_radius_from_deposit(per_asset, n_assets).unwrap();
    let sphere = Sphere {
        radius,
        n: n_assets,
    };

    let reserves: Vec<FixedPoint> = reserves_u64
        .iter()
        .map(|&r| FixedPoint::checked_from_u64(r).unwrap())
        .collect();

    let amount_in_fp = FixedPoint::checked_from_u64(amount_in).unwrap();
    let fee = compute_fee(amount_in_fp, fee_rate_bps).unwrap();
    let net_in = amount_in_fp.checked_sub(fee).unwrap();

    let expected_out_fp =
        compute_amount_out_analytical(&sphere, &reserves, token_in, token_out, net_in).unwrap();

    expected_out_fp.to_u64().unwrap()
}

// ── Instruction Builders ──

fn build_execute_swap_data(
    token_in_index: u8,
    token_out_index: u8,
    amount_in: u64,
    expected_amount_out: u64,
    min_amount_out: u64,
) -> Vec<u8> {
    let disc = anchor_discriminator("global:execute_swap");
    let mut data = Vec::new();
    data.extend_from_slice(&disc);
    data.push(token_in_index);
    data.push(token_out_index);
    data.extend_from_slice(&amount_in.to_le_bytes());
    data.extend_from_slice(&expected_amount_out.to_le_bytes());
    data.extend_from_slice(&min_amount_out.to_le_bytes());
    data
}

// ── Test Scaffolding ──

struct TestPool {
    svm: LiteSVM,
    authority: Keypair,
    pool_pda: Pubkey,
    mints: Vec<Keypair>,
    vault_pdas: Vec<Pubkey>,
    #[allow(dead_code)]
    authority_atas: Vec<Pubkey>,
    n_assets: u8,
    deposit: u64,
}

/// Initialize a 3-asset pool with the given deposit per asset and fee_rate_bps = 30.
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

/// Create a swapper with funded ATAs for all pool tokens.
fn setup_swapper(tp: &mut TestPool, fund_amount: u64) -> (Keypair, Vec<Pubkey>) {
    let swapper = Keypair::new();
    tp.svm
        .airdrop(&swapper.pubkey(), 5_000_000_000)
        .unwrap();

    let mut swapper_atas = Vec::new();
    for mint_kp in &tp.mints {
        let ata = create_ata(&mut tp.svm, &tp.authority, &mint_kp.pubkey(), &swapper.pubkey());
        mint_to(
            &mut tp.svm,
            &tp.authority,
            &mint_kp.pubkey(),
            &ata,
            fund_amount,
        );
        swapper_atas.push(ata);
    }

    (swapper, swapper_atas)
}

/// Build and send an execute_swap instruction. Returns Ok(()) or Err.
fn send_swap(
    tp: &mut TestPool,
    swapper: &Keypair,
    swapper_atas: &[Pubkey],
    token_in: usize,
    token_out: usize,
    amount_in: u64,
    expected_amount_out: u64,
    min_amount_out: u64,
) -> Result<(), String> {
    let data = build_execute_swap_data(
        token_in as u8,
        token_out as u8,
        amount_in,
        expected_amount_out,
        min_amount_out,
    );

    // Accounts: [user, pool, token_program, ...remaining_accounts]
    // remaining_accounts: [vault_in, vault_out, user_ata_in, user_ata_out]
    let accounts = vec![
        AccountMeta::new(swapper.pubkey(), true),
        AccountMeta::new(tp.pool_pda, false),
        AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false),
        // remaining_accounts
        AccountMeta::new(tp.vault_pdas[token_in], false),
        AccountMeta::new(tp.vault_pdas[token_out], false),
        AccountMeta::new(swapper_atas[token_in], false),
        AccountMeta::new(swapper_atas[token_out], false),
    ];

    let ix = Instruction {
        program_id: PROGRAM_ID,
        accounts,
        data,
    };

    let blockhash = tp.svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&swapper.pubkey()),
        &[swapper],
        blockhash,
    );

    tp.svm
        .send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

// ══════════════════════════════════════════════
// Test 1: swap transfers tokens correctly
// ══════════════════════════════════════════════

#[test]
fn test_swap_transfers_tokens() {
    let mut tp = setup_pool(1_000_000);

    // Create swapper with 100K of each token
    let fund_amount: u64 = 100_000;
    let (swapper, swapper_atas) = setup_swapper(&mut tp, fund_amount);

    // Compute exact expected_amount_out using Q64.64 math
    let amount_in: u64 = 10_000;
    let expected_out = compute_valid_expected_out(
        tp.n_assets, tp.deposit, 30, 0, 1, amount_in,
    );
    assert!(expected_out > 0, "expected_out must be positive");

    // Record pre-swap balances
    let vault_in_before = read_token_amount(&tp.svm, &tp.vault_pdas[0]);
    let vault_out_before = read_token_amount(&tp.svm, &tp.vault_pdas[1]);
    let user_ata_in_before = read_token_amount(&tp.svm, &swapper_atas[0]);
    let user_ata_out_before = read_token_amount(&tp.svm, &swapper_atas[1]);

    // Execute swap: token 0 → token 1
    send_swap(
        &mut tp, &swapper, &swapper_atas,
        0, 1,
        amount_in, expected_out, 1,
    )
    .expect("swap should succeed");

    // Verify vault balances
    let vault_in_after = read_token_amount(&tp.svm, &tp.vault_pdas[0]);
    let vault_out_after = read_token_amount(&tp.svm, &tp.vault_pdas[1]);
    assert_eq!(
        vault_in_after,
        vault_in_before + amount_in,
        "vault_in should receive amount_in"
    );
    assert_eq!(
        vault_out_after,
        vault_out_before - expected_out,
        "vault_out should send expected_out"
    );

    // Verify user ATA balances
    let user_ata_in_after = read_token_amount(&tp.svm, &swapper_atas[0]);
    let user_ata_out_after = read_token_amount(&tp.svm, &swapper_atas[1]);
    assert_eq!(
        user_ata_in_after,
        user_ata_in_before - amount_in,
        "user should spend amount_in"
    );
    assert_eq!(
        user_ata_out_after,
        user_ata_out_before + expected_out,
        "user should receive expected_out"
    );

    // Verify uninvolved vault unchanged
    let vault_2_after = read_token_amount(&tp.svm, &tp.vault_pdas[2]);
    assert_eq!(
        vault_2_after, tp.deposit,
        "uninvolved vault should be unchanged"
    );
}

// ══════════════════════════════════════════════
// Test 2: swap rejects same-token swap
// ══════════════════════════════════════════════

#[test]
fn test_swap_rejects_same_token() {
    let mut tp = setup_pool(1_000_000);
    let fund_amount: u64 = 100_000;
    let (swapper, swapper_atas) = setup_swapper(&mut tp, fund_amount);

    let result = send_swap(
        &mut tp, &swapper, &swapper_atas,
        0, 0, // same token
        10_000, 9_000, 1,
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_SAME_TOKEN_SWAP),
        "expected SameTokenSwap (6011), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 3: swap rejects when min_amount_out exceeds actual output (slippage)
// ══════════════════════════════════════════════

#[test]
fn test_swap_rejects_slippage_exceeded() {
    let mut tp = setup_pool(1_000_000);
    let fund_amount: u64 = 100_000;
    let (swapper, swapper_atas) = setup_swapper(&mut tp, fund_amount);

    // Compute the real expected output for reference
    let amount_in: u64 = 10_000;
    let expected_out = compute_valid_expected_out(
        tp.n_assets, tp.deposit, 30, 0, 1, amount_in,
    );

    // Set min_amount_out higher than actual → slippage exceeded
    let result = send_swap(
        &mut tp, &swapper, &swapper_atas,
        0, 1,
        amount_in,
        expected_out,
        expected_out + 1_000, // min_out > actual → must reject
    );
    let err = result.unwrap_err();
    assert_eq!(
        extract_anchor_error_code(&err),
        Some(ERROR_SLIPPAGE_EXCEEDED),
        "expected SlippageExceeded (6010), got: {err}"
    );
}

// ══════════════════════════════════════════════
// Test 4: swap roundtrip (swap A→B then B→A)
// ══════════════════════════════════════════════

#[test]
fn test_swap_roundtrip() {
    let mut tp = setup_pool(1_000_000);

    let fund_amount: u64 = 100_000;
    let (swapper, swapper_atas) = setup_swapper(&mut tp, fund_amount);

    let user_ata0_initial = read_token_amount(&tp.svm, &swapper_atas[0]);
    let user_ata1_initial = read_token_amount(&tp.svm, &swapper_atas[1]);

    // ── First swap: 0 → 1 ──
    let amount_in_1: u64 = 10_000;
    let expected_out_1 = compute_valid_expected_out(
        tp.n_assets, tp.deposit, 30, 0, 1, amount_in_1,
    );

    send_swap(
        &mut tp, &swapper, &swapper_atas,
        0, 1,
        amount_in_1, expected_out_1, 1,
    )
    .expect("first swap should succeed");

    // ── Second swap: 1 → 0 (reverse) ──
    // Read updated vault balances to determine new reserves
    let reserve_0 = read_token_amount(&tp.svm, &tp.vault_pdas[0]);
    let reserve_1 = read_token_amount(&tp.svm, &tp.vault_pdas[1]);
    let reserve_2 = read_token_amount(&tp.svm, &tp.vault_pdas[2]);

    // Swap back the received amount
    let amount_in_2 = expected_out_1;
    let expected_out_2 = compute_expected_out_with_reserves(
        tp.n_assets, tp.deposit, 30,
        &[reserve_0, reserve_1, reserve_2],
        1, 0,
        amount_in_2,
    );

    send_swap(
        &mut tp, &swapper, &swapper_atas,
        1, 0,
        amount_in_2, expected_out_2, 1,
    )
    .expect("reverse swap should succeed");

    // ── Verify approximate restoration ──
    let user_ata0_final = read_token_amount(&tp.svm, &swapper_atas[0]);
    let user_ata1_final = read_token_amount(&tp.svm, &swapper_atas[1]);

    // User should have approximately the same token 0 balance (minus fee losses)
    // Fee is ~30 bps each way, so total loss ≈ 60 bps of 10K ≈ 6 tokens
    let token0_loss = user_ata0_initial - user_ata0_final;
    assert!(
        token0_loss < 100, // generous bound for fee + rounding
        "token 0 loss should be small (fee only), got {}",
        token0_loss
    );

    // Token 1 should be approximately unchanged
    let token1_diff = if user_ata1_final >= user_ata1_initial {
        user_ata1_final - user_ata1_initial
    } else {
        user_ata1_initial - user_ata1_final
    };
    assert!(
        token1_diff < 100,
        "token 1 balance should be approximately unchanged, diff = {}",
        token1_diff
    );
}
