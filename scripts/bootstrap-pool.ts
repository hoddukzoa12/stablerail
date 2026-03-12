/**
 * bootstrap-pool.ts — Devnet pool bootstrap for Orbital Settlement Protocol
 *
 * Steps:
 *   1. Creates 3 mock SPL token mints (mock-USDC, mock-USDT, mock-PYUSD)
 *   2. Creates deployer ATAs and mints initial supply
 *   3. Calls initialize_pool (3-asset, 30bps fee, 1000 tokens/asset)
 *   4. Calls create_policy (100K max trade, 1M daily volume)
 *   5. Calls manage_allowlist (adds deployer as executor)
 *   6. Writes devnet-config.json
 *
 * Usage:
 *   cd scripts && npm install && npm run bootstrap
 *
 * Idempotency:
 *   Each step checks if the on-chain account already exists and skips if so.
 *   Safe to re-run after partial failures.
 */

import * as anchor from "@coral-xyz/anchor";
// @ts-ignore — bn.js has no type declarations in this isolated scripts package
import BN from "bn.js";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";

// ────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────

const PROGRAM_ID = new PublicKey(
  "C7dFX4QVV8QCdzP4fZi3Vcx8oP1cYhTaXD7kvvat8W1w"
);
const DEVNET_RPC = "https://api.devnet.solana.com";

const N_ASSETS = 3;
const FEE_RATE_BPS = 1;
const DECIMALS = 6;
// 100 tokens at 6 decimals (Q64.64 safe range for checked_mul)
const INITIAL_DEPOSIT_PER_ASSET = BigInt(100_000_000);
// 2x deposit for swap buffer
const MINT_AMOUNT_PER_ASSET = BigInt(200_000_000);
// Policy: 100K tokens per trade, 1M tokens/day
const MAX_TRADE_AMOUNT = BigInt(100_000_000_000);
const MAX_DAILY_VOLUME = BigInt(1_000_000_000_000);

const TOKEN_SYMBOLS = ["mock-USDC", "mock-USDT", "mock-PYUSD"] as const;

// ────────────────────────────────────────────
// Paths
// ────────────────────────────────────────────

const SCRIPT_DIR = path.dirname(new URL(import.meta.url).pathname ?? ".");
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const IDL_PATH = path.join(ROOT_DIR, "anchor/target/idl/orbital.json");
const WALLET_PATH = path.join(
  process.env.HOME ?? "~",
  ".config/solana/id.json"
);
const CONFIG_OUTPUT_PATH = path.join(SCRIPT_DIR, "devnet-config.json");

const MOCK_MINT_KEYPAIR_PATHS = [
  path.join(SCRIPT_DIR, "mock-usdc-mint.json"),
  path.join(SCRIPT_DIR, "mock-usdt-mint.json"),
  path.join(SCRIPT_DIR, "mock-pyusd-mint.json"),
];

// ────────────────────────────────────────────
// PDA derivation (mirrors on-chain seeds)
// ────────────────────────────────────────────

function derivePoolPda(authority: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), authority.toBuffer()],
    PROGRAM_ID
  );
}

function deriveVaultPda(pool: PublicKey, mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), pool.toBuffer(), mint.toBuffer()],
    PROGRAM_ID
  );
}

function derivePolicyPda(
  pool: PublicKey,
  authority: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("policy"), pool.toBuffer(), authority.toBuffer()],
    PROGRAM_ID
  );
}

function deriveAllowlistPda(policy: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("allowlist"), policy.toBuffer()],
    PROGRAM_ID
  );
}

// ────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────

function loadKeypair(filePath: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(filePath, "utf-8")) as number[];
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

function loadOrCreateKeypair(filePath: string): Keypair {
  if (fs.existsSync(filePath)) {
    return loadKeypair(filePath);
  }
  const kp = Keypair.generate();
  fs.writeFileSync(filePath, JSON.stringify(Array.from(kp.secretKey)));
  console.log(`  Generated keypair: ${path.basename(filePath)}`);
  return kp;
}

async function accountExists(
  connection: Connection,
  address: PublicKey
): Promise<boolean> {
  const info = await connection.getAccountInfo(address);
  return info !== null;
}

// ────────────────────────────────────────────
// Main
// ────────────────────────────────────────────

async function main() {
  console.log("=== Orbital Bootstrap ===");
  console.log(`Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`RPC:     ${DEVNET_RPC}`);
  console.log("");

  // 1. Connection + deployer
  const connection = new Connection(DEVNET_RPC, "confirmed");
  if (!fs.existsSync(WALLET_PATH)) {
    throw new Error(
      `Deployer keypair not found at ${WALLET_PATH}.\n` +
        "Run scripts/deploy-devnet.sh first."
    );
  }
  const deployer = loadKeypair(WALLET_PATH);
  console.log(`Deployer: ${deployer.publicKey.toBase58()}`);

  const balance = await connection.getBalance(deployer.publicKey);
  console.log(`Balance:  ${(balance / 1e9).toFixed(3)} SOL`);
  if (balance < 0.1e9) {
    throw new Error("Balance too low (< 0.1 SOL). Run deploy-devnet.sh.");
  }

  // 2. Anchor provider + program
  const wallet = new anchor.Wallet(deployer);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf-8"));
  // Anchor 0.31: new Program(idl, provider) — programId from idl.address
  const program = new anchor.Program(idl, provider);

  // 3. Mock mints
  console.log("\n[1/5] Setting up mock token mints...");
  const mintKeypairs = MOCK_MINT_KEYPAIR_PATHS.map(loadOrCreateKeypair);
  const mintPubkeys: PublicKey[] = [];

  for (let i = 0; i < N_ASSETS; i++) {
    const kp = mintKeypairs[i];
    if (await accountExists(connection, kp.publicKey)) {
      console.log(`  ${TOKEN_SYMBOLS[i]}: ${kp.publicKey.toBase58()} (exists)`);
    } else {
      await createMint(
        connection,
        deployer,
        deployer.publicKey,
        null,
        DECIMALS,
        kp
      );
      console.log(
        `  ${TOKEN_SYMBOLS[i]}: ${kp.publicKey.toBase58()} (created)`
      );
    }
    mintPubkeys.push(kp.publicKey);
  }

  // 4. ATAs + mint supply
  console.log("\n[2/5] Creating ATAs and minting tokens...");
  const ataAddresses: PublicKey[] = [];

  for (let i = 0; i < N_ASSETS; i++) {
    const ata = await getOrCreateAssociatedTokenAccount(
      connection,
      deployer,
      mintPubkeys[i],
      deployer.publicKey
    );

    if (ata.amount < INITIAL_DEPOSIT_PER_ASSET) {
      const toMint = MINT_AMOUNT_PER_ASSET - ata.amount;
      await mintTo(
        connection,
        deployer,
        mintPubkeys[i],
        ata.address,
        deployer,
        toMint
      );
      console.log(
        `  ${TOKEN_SYMBOLS[i]}: minted ${toMint} to ${ata.address.toBase58()}`
      );
    } else {
      console.log(
        `  ${TOKEN_SYMBOLS[i]}: sufficient balance (${ata.amount})`
      );
    }
    ataAddresses.push(ata.address);
  }

  // 5. Derive PDAs
  const [poolPda] = derivePoolPda(deployer.publicKey);
  const vaultPdas = mintPubkeys.map((m) => deriveVaultPda(poolPda, m)[0]);
  const [policyPda] = derivePolicyPda(poolPda, deployer.publicKey);
  const [allowlistPda] = deriveAllowlistPda(policyPda);

  console.log("\nPDAs:");
  console.log(`  Pool:      ${poolPda.toBase58()}`);
  vaultPdas.forEach((v, i) =>
    console.log(`  Vault[${i}]:  ${v.toBase58()}`)
  );
  console.log(`  Policy:    ${policyPda.toBase58()}`);
  console.log(`  Allowlist: ${allowlistPda.toBase58()}`);

  // 6. initialize_pool
  console.log("\n[3/5] Initializing pool...");
  if (await accountExists(connection, poolPda)) {
    console.log("  Pool already initialized — skipping");
  } else {
    // Pad token_mints to [Pubkey; 8]
    const tokenMintsArg = [
      ...mintPubkeys,
      ...Array(8 - N_ASSETS).fill(PublicKey.default),
    ];

    // remaining_accounts: [mints(ro), vaults(rw), atas(rw)]
    const remainingAccounts = [
      ...mintPubkeys.map((pk) => ({
        pubkey: pk,
        isSigner: false,
        isWritable: false,
      })),
      ...vaultPdas.map((pk) => ({
        pubkey: pk,
        isSigner: false,
        isWritable: true,
      })),
      ...ataAddresses.map((pk) => ({
        pubkey: pk,
        isSigner: false,
        isWritable: true,
      })),
    ];

    const tx = await program.methods
      .initializePool({
        nAssets: N_ASSETS,
        feeRateBps: FEE_RATE_BPS,
        initialDepositPerAsset: new BN(
          INITIAL_DEPOSIT_PER_ASSET.toString()
        ),
        tokenMints: tokenMintsArg,
      })
      .accounts({
        authority: deployer.publicKey,
        pool: poolPda,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .remainingAccounts(remainingAccounts)
      .signers([deployer])
      .rpc();

    console.log(`  Pool initialized: ${tx}`);
  }

  // 7. create_policy
  console.log("\n[4/5] Creating policy...");
  if (await accountExists(connection, policyPda)) {
    console.log("  Policy already exists — skipping");
  } else {
    const tx = await program.methods
      .createPolicy({
        maxTradeAmount: new BN(MAX_TRADE_AMOUNT.toString()),
        maxDailyVolume: new BN(MAX_DAILY_VOLUME.toString()),
      })
      .accounts({
        authority: deployer.publicKey,
        pool: poolPda,
        policy: policyPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([deployer])
      .rpc();

    console.log(`  Policy created: ${tx}`);
  }

  // 8. manage_allowlist (add deployer)
  console.log("\n[5/5] Adding deployer to allowlist...");
  if (await accountExists(connection, allowlistPda)) {
    console.log("  Allowlist exists — checking membership...");
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const data = await (program.account as any).allowlistState.fetch(allowlistPda);
      const count = data.count as number;
      const addresses = (data.addresses as PublicKey[]).slice(
        0,
        count
      );
      const found = addresses.some(
        (a) => a.toBase58() === deployer.publicKey.toBase58()
      );
      if (found) {
        console.log("  Deployer already in allowlist — skipping");
      } else {
        await callManageAllowlist(program, deployer, policyPda, allowlistPda);
      }
    } catch (err) {
      console.warn("  Could not fetch allowlist state, retrying add:", err);
      try {
        await callManageAllowlist(program, deployer, policyPda, allowlistPda);
      } catch {
        console.log("  Deployer likely already in allowlist — continuing");
      }
    }
  } else {
    await callManageAllowlist(program, deployer, policyPda, allowlistPda);
  }

  // 9. Write config
  const config = {
    network: "devnet",
    rpcUrl: DEVNET_RPC,
    programId: PROGRAM_ID.toBase58(),
    deployer: deployer.publicKey.toBase58(),
    pool: poolPda.toBase58(),
    policy: policyPda.toBase58(),
    allowlist: allowlistPda.toBase58(),
    mints: Object.fromEntries(
      TOKEN_SYMBOLS.map((sym, i) => [sym, mintPubkeys[i].toBase58()])
    ),
    vaults: Object.fromEntries(
      TOKEN_SYMBOLS.map((sym, i) => [sym, vaultPdas[i].toBase58()])
    ),
    params: {
      nAssets: N_ASSETS,
      feeRateBps: FEE_RATE_BPS,
      decimals: DECIMALS,
      initialDepositPerAsset: INITIAL_DEPOSIT_PER_ASSET.toString(),
      maxTradeAmount: MAX_TRADE_AMOUNT.toString(),
      maxDailyVolume: MAX_DAILY_VOLUME.toString(),
    },
    generatedAt: new Date().toISOString(),
  };

  fs.writeFileSync(CONFIG_OUTPUT_PATH, JSON.stringify(config, null, 2) + "\n");

  console.log("\n=== Bootstrap complete ===");
  console.log(`Config: ${CONFIG_OUTPUT_PATH}`);
  console.log(JSON.stringify(config, null, 2));
}

async function callManageAllowlist(
  program: anchor.Program,
  deployer: Keypair,
  policyPda: PublicKey,
  allowlistPda: PublicKey
): Promise<void> {
  const tx = await program.methods
    .manageAllowlist({
      action: { add: {} },
      address: deployer.publicKey,
    })
    .accounts({
      authority: deployer.publicKey,
      policy: policyPda,
      allowlist: allowlistPda,
      systemProgram: SystemProgram.programId,
    })
    .signers([deployer])
    .rpc();

  console.log(`  Deployer added to allowlist: ${tx}`);
}

main().catch((err) => {
  console.error("\nBootstrap failed:", err);
  process.exit(1);
});
