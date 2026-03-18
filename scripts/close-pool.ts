/**
 * close-pool.ts — Close existing devnet pool to allow re-initialization
 *
 * Calls the on-chain close_pool instruction which:
 *   1. Closes all vault token accounts (returns tokens + rent to authority ATAs)
 *   2. Closes the pool PDA (returns rent lamports to authority)
 *
 * Usage:
 *   cd scripts && npx tsx close-pool.ts
 */

import * as anchor from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
} from "@solana/web3.js";
import {
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as fs from "fs";
import * as path from "path";

// ────────────────────────────────────────────
// Constants
// ────────────────────────────────────────────

const PROGRAM_ID = new PublicKey(
  "BZDXfJTBpH9ZMo2dz57BFKGNw4FYFCDr1KaUUkFtfRVD"
);
const DEVNET_RPC = "https://api.devnet.solana.com";

const SCRIPT_DIR = path.dirname(new URL(import.meta.url).pathname ?? ".");
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const IDL_PATH = path.join(ROOT_DIR, "anchor/target/idl/orbital.json");
const HOME = process.env.HOME;
if (!HOME) {
  throw new Error("HOME environment variable is not set.");
}
const WALLET_PATH = path.join(HOME, ".config/solana/id.json");

// ────────────────────────────────────────────
// PDA derivation
// ────────────────────────────────────────────

function derivePoolPda(authority: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), authority.toBuffer()],
    PROGRAM_ID
  );
}

// ────────────────────────────────────────────
// Main
// ────────────────────────────────────────────

async function main() {
  console.log("=== Close Pool ===\n");

  // Load wallet
  const walletRaw = JSON.parse(fs.readFileSync(WALLET_PATH, "utf-8"));
  const authority = Keypair.fromSecretKey(Uint8Array.from(walletRaw));
  const connection = new Connection(DEVNET_RPC, "confirmed");

  console.log(`Authority: ${authority.publicKey.toBase58()}`);

  // Load IDL
  const idlRaw = JSON.parse(fs.readFileSync(IDL_PATH, "utf-8"));
  const provider = new anchor.AnchorProvider(
    connection,
    new anchor.Wallet(authority),
    { commitment: "confirmed" }
  );
  const program = new anchor.Program(idlRaw, provider);

  // Derive pool PDA
  const [poolPda] = derivePoolPda(authority.publicKey);
  console.log(`Pool PDA: ${poolPda.toBase58()}`);

  // Read pool state to get vault and mint info
  let poolState: any;
  try {
    poolState = await (program.account as any).poolState.fetch(poolPda);
  } catch {
    console.log("Pool does not exist — nothing to close.");
    return;
  }

  const nAssets = poolState.nAssets as number;
  console.log(`Assets: ${nAssets}`);

  // Build remaining_accounts: [vault0, vault1, ..., ata0, ata1, ...]
  const remainingAccounts: anchor.web3.AccountMeta[] = [];
  const vaultKeys: PublicKey[] = [];
  const ataKeys: PublicKey[] = [];

  // First pass: collect vaults
  for (let i = 0; i < nAssets; i++) {
    const vaultKey = poolState.tokenVaults[i] as PublicKey;
    vaultKeys.push(vaultKey);
    remainingAccounts.push({
      pubkey: vaultKey,
      isSigner: false,
      isWritable: true,
    });
  }

  // Second pass: create/get authority ATAs and add
  for (let i = 0; i < nAssets; i++) {
    const mintKey = poolState.tokenMints[i] as PublicKey;
    const ata = await getOrCreateAssociatedTokenAccount(
      connection,
      authority,
      mintKey,
      authority.publicKey
    );
    ataKeys.push(ata.address);
    remainingAccounts.push({
      pubkey: ata.address,
      isSigner: false,
      isWritable: true,
    });
  }

  console.log("\nVaults:");
  for (let i = 0; i < nAssets; i++) {
    console.log(`  [${i}] ${vaultKeys[i].toBase58()}`);
  }
  console.log("Destination ATAs:");
  for (let i = 0; i < nAssets; i++) {
    console.log(`  [${i}] ${ataKeys[i].toBase58()}`);
  }

  // Call close_pool
  console.log("\nClosing pool...");
  const tx = await (program.methods as any)
    .closePool()
    .accounts({
      authority: authority.publicKey,
      pool: poolPda,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .remainingAccounts(remainingAccounts)
    .rpc();

  console.log(`\n✅ Pool closed! tx: ${tx}`);
  console.log("\nNext: npm run bootstrap");
}

main().catch((err) => {
  console.error("ERROR:", err);
  process.exit(1);
});
