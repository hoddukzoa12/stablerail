/**
 * create-demo-ticks.ts — Create demo tick accounts on devnet for hackathon demo
 *
 * Creates concentrated liquidity ticks at various capital efficiency levels
 * to showcase Orbital's superiority over Curve (full-range) and Uniswap V3.
 *
 * Tick math (spherical cap):
 *   k_min = r * (√n - 1)       — widest cap, lowest efficiency
 *   k_max = r * (n-1) / √n     — narrowest valid cap, highest efficiency
 *   capital_efficiency = x_base / (x_base - x_min)
 *
 * Demo strategy:
 *   - 5 ticks at 15%, 30%, 50%, 70%, 90% of k_range → increasing efficiency
 *   - Optionally add liquidity to each tick (concentrated deposits)
 *
 * Usage:
 *   cd scripts && npx tsx create-demo-ticks.ts
 *   cd scripts && npx tsx create-demo-ticks.ts --with-liquidity
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
  "B8n8gTjF5A4d6tmVFnz5ZCY3bKy5cfEQ1ZhYogbSxbfU"
);
const DEVNET_RPC = "https://api.devnet.solana.com";

const SCRIPT_DIR = path.dirname(new URL(import.meta.url).pathname ?? ".");
const ROOT_DIR = path.resolve(SCRIPT_DIR, "..");
const IDL_PATH = path.join(ROOT_DIR, "anchor/target/idl/orbital.json");
const HOME = process.env.HOME;
if (!HOME) throw new Error("HOME not set");
const WALLET_PATH = path.join(HOME, ".config/solana/id.json");
const CONFIG_PATH = path.join(SCRIPT_DIR, "devnet-config.json");

// Q64.64 shift
const Q64 = BigInt(1) << BigInt(64);

// Demo tick positions: fraction of k_range from k_min to k_max
// Lower fraction → closer to k_min → wider cap → lower efficiency
// Higher fraction → closer to k_max → narrower cap → higher efficiency
const DEMO_TICK_FRACTIONS = [0.15, 0.30, 0.50, 0.70, 0.90];
const DEMO_TICK_LABELS = [
  "Wide (low concentration)",
  "Moderate-wide",
  "Balanced (medium concentration)",
  "Moderate-narrow",
  "Narrow (high concentration)",
];

// Per-tick liquidity deposit: $2M per asset (total $6M per tick, $30M across 5 ticks)
const LIQUIDITY_PER_ASSET = BigInt(2_000_000_000_000); // 2M at 6 decimals
// Extra mint amount per token for liquidity deposits
const EXTRA_MINT_PER_ASSET = BigInt(15_000_000_000_000); // 15M at 6 decimals (buffer)

// ────────────────────────────────────────────
// Q64.64 Math Helpers (mirrors on-chain FixedPoint)
// ────────────────────────────────────────────

function fpFromFloat(x: number): bigint {
  return BigInt(Math.round(x * Number(Q64)));
}

function fpToFloat(raw: bigint): number {
  return Number(raw) / Number(Q64);
}

function fpMul(a: bigint, b: bigint): bigint {
  return (a * b) / Q64;
}

function fpDiv(a: bigint, b: bigint): bigint {
  return (a * Q64) / b;
}

function fpSqrt(x: bigint): bigint {
  // Newton's method for Q64.64 sqrt
  if (x <= BigInt(0)) return BigInt(0);
  // Initial guess: sqrt(x_float) in Q64.64
  const floatGuess = Math.sqrt(Number(x) / Number(Q64));
  let guess = BigInt(Math.round(floatGuess * Number(Q64)));
  if (guess <= BigInt(0)) guess = BigInt(1);

  for (let i = 0; i < 20; i++) {
    const next = (guess + fpDiv(x, guess)) / BigInt(2);
    if (next === guess || next === guess - BigInt(1) || next === guess + BigInt(1)) break;
    guess = next;
  }
  return guess;
}

// ────────────────────────────────────────────
// PDA derivation
// ────────────────────────────────────────────

function derivePoolPda(authority: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("pool"), authority.toBuffer()],
    PROGRAM_ID
  );
}

function deriveTickPda(pool: PublicKey, tickIndex: number): [PublicKey, number] {
  const buf = Buffer.alloc(2);
  buf.writeUInt16LE(tickIndex);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("tick"), pool.toBuffer(), buf],
    PROGRAM_ID
  );
}

function derivePositionPda(
  pool: PublicKey,
  provider: PublicKey,
  positionIndex: bigint
): [PublicKey, number] {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(positionIndex);
  return PublicKey.findProgramAddressSync(
    [Buffer.from("position"), pool.toBuffer(), provider.toBuffer(), buf],
    PROGRAM_ID
  );
}

function deriveVaultPda(pool: PublicKey, mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), pool.toBuffer(), mint.toBuffer()],
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

function toBN128(val: bigint): BN {
  // Convert bigint to BN for Anchor i128 serialization
  // i128 is signed — handle negative values
  if (val >= BigInt(0)) {
    return new BN(val.toString());
  }
  // Negative: two's complement for 128-bit
  const twosComp = (BigInt(1) << BigInt(128)) + val;
  return new BN(twosComp.toString());
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
  const withLiquidity = process.argv.includes("--with-liquidity");

  console.log("=== Orbital Demo Tick Creator ===");
  console.log(`Program: ${PROGRAM_ID.toBase58()}`);
  console.log(`Mode:    ${withLiquidity ? "Create ticks + Add liquidity" : "Create ticks only"}`);
  console.log("");

  // 1. Connection + deployer
  const connection = new Connection(DEVNET_RPC, "confirmed");
  const deployer = loadKeypair(WALLET_PATH);
  console.log(`Deployer: ${deployer.publicKey.toBase58()}`);

  const balance = await connection.getBalance(deployer.publicKey);
  console.log(`Balance:  ${(balance / 1e9).toFixed(3)} SOL\n`);
  if (balance < 0.05e9) {
    throw new Error("Balance too low (< 0.05 SOL)");
  }

  // 2. Anchor setup
  const wallet = new anchor.Wallet(deployer);
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  const idl = JSON.parse(fs.readFileSync(IDL_PATH, "utf-8"));
  const program = new anchor.Program(idl, provider);

  // 3. Read pool state
  const [poolPda] = derivePoolPda(deployer.publicKey);
  console.log(`Pool: ${poolPda.toBase58()}`);

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const poolState = await (program.account as any).poolState.fetch(poolPda);
  const nAssets = poolState.nAssets as number;
  const tickCount = poolState.tickCount as number;

  // Extract sphere radius from on-chain state (Q64.64 raw i128)
  // Anchor deserializes i128 as BN
  const radiusRaw = BigInt((poolState.sphere.radius.raw as BN).toString());
  const radiusFloat = fpToFloat(radiusRaw);

  console.log(`N assets:   ${nAssets}`);
  console.log(`Tick count: ${tickCount}`);
  console.log(`Radius:     ${radiusFloat.toFixed(2)} (raw: ${radiusRaw})`);

  // 4. Compute k bounds
  const n = BigInt(nAssets);
  const sqrtN = fpSqrt(n * Q64); // sqrt(n) in Q64.64

  // k_min = r * (sqrt(n) - 1)
  const kMin = fpMul(radiusRaw, sqrtN - Q64);
  // k_max = r * (n - 1) / sqrt(n)
  const kMax = fpDiv(fpMul(radiusRaw, (n - BigInt(1)) * Q64), sqrtN);

  const kMinFloat = fpToFloat(kMin);
  const kMaxFloat = fpToFloat(kMax);

  console.log(`\nk bounds (Q64.64):`);
  console.log(`  k_min: ${kMinFloat.toFixed(2)}`);
  console.log(`  k_max: ${kMaxFloat.toFixed(2)}`);
  console.log(`  range: ${(kMaxFloat - kMinFloat).toFixed(2)}`);

  // 5. Compute demo k values
  const kRange = kMax - kMin;
  const demoKValues: bigint[] = [];

  console.log(`\nDemo ticks (${DEMO_TICK_FRACTIONS.length} ticks):`);
  console.log("─".repeat(80));

  for (let i = 0; i < DEMO_TICK_FRACTIONS.length; i++) {
    const frac = DEMO_TICK_FRACTIONS[i];
    // k = k_min + frac * (k_max - k_min)
    // Add small epsilon to avoid exact boundary
    const epsilon = kRange / BigInt(10000);
    const k = kMin + BigInt(Math.round(Number(kRange) * frac)) + epsilon;
    demoKValues.push(k);

    const kFloat = fpToFloat(k);

    // Compute capital efficiency preview
    const xBase = fpMul(radiusRaw, Q64 - fpDiv(Q64, sqrtN)); // r * (1 - 1/sqrt(n))
    // Simplified efficiency estimate
    const nNum = Number(n);
    const kNum = kFloat;
    const rNum = radiusFloat;
    const sqrtNNum = Math.sqrt(nNum);
    const disc = Math.sqrt(
      kNum * kNum * nNum - nNum * Math.pow((nNum - 1) * rNum - kNum * sqrtNNum, 2)
    );
    const xMinNum = (kNum * sqrtNNum - disc) / nNum;
    const xBaseNum = rNum * (1 - 1 / sqrtNNum);
    const efficiency = xBaseNum / (xBaseNum - xMinNum);

    console.log(
      `  [${i}] ${DEMO_TICK_LABELS[i]}` +
        `\n      k = ${kFloat.toFixed(2)} (${(frac * 100).toFixed(0)}% of range)` +
        `\n      capital efficiency ≈ ${efficiency.toFixed(2)}x` +
        `\n      x_min ≈ ${xMinNum.toFixed(2)}, x_base = ${xBaseNum.toFixed(2)}`
    );
  }

  // 6. Create ticks
  console.log(`\n${"═".repeat(80)}`);
  console.log("Creating ticks on-chain...\n");

  const createdTicks: { index: number; pda: PublicKey; k: bigint }[] = [];

  for (let i = 0; i < demoKValues.length; i++) {
    const currentTickCount = tickCount + i;
    const [tickPda] = deriveTickPda(poolPda, currentTickCount);

    if (await accountExists(connection, tickPda)) {
      console.log(`  Tick[${currentTickCount}] already exists at ${tickPda.toBase58()} — skipping`);
      createdTicks.push({ index: currentTickCount, pda: tickPda, k: demoKValues[i] });
      continue;
    }

    console.log(`  Creating tick[${currentTickCount}]: ${DEMO_TICK_LABELS[i]}...`);

    try {
      const tx = await program.methods
        .createTick({
          kRaw: toBN128(demoKValues[i]),
        })
        .accounts({
          creator: deployer.publicKey,
          pool: poolPda,
          tick: tickPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([deployer])
        .rpc();

      console.log(`    ✓ tx: ${tx}`);
      createdTicks.push({ index: currentTickCount, pda: tickPda, k: demoKValues[i] });
    } catch (err) {
      console.error(`    ✗ Failed: ${err}`);
      throw err;
    }
  }

  console.log(`\nCreated ${createdTicks.length} ticks.`);

  // 7. Optionally add liquidity to each tick
  if (withLiquidity) {
    console.log(`\n${"═".repeat(80)}`);
    console.log("Adding concentrated liquidity to ticks...\n");

    // Get mints and vaults from config
    const config = JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8"));
    const mintKeys = Object.values(config.mints).map(
      (m) => new PublicKey(m as string)
    );
    const vaultKeys = mintKeys.map((m) => deriveVaultPda(poolPda, m)[0]);

    // Get deployer ATAs and mint extra tokens if needed
    const ataAddresses: PublicKey[] = [];
    const totalNeeded = LIQUIDITY_PER_ASSET * BigInt(createdTicks.length);
    for (const mint of mintKeys) {
      const ata = await getOrCreateAssociatedTokenAccount(
        connection,
        deployer,
        mint,
        deployer.publicKey
      );
      if (ata.amount < totalNeeded) {
        const toMint = EXTRA_MINT_PER_ASSET;
        console.log(`  Minting ${Number(toMint) / 1e6}M extra tokens for ${mint.toBase58().slice(0, 8)}...`);
        await mintTo(connection, deployer, mint, ata.address, deployer, toMint);
      }
      ataAddresses.push(ata.address);
    }

    // Re-fetch pool state to get current position_count
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const freshPool = await (program.account as any).poolState.fetch(poolPda);
    let positionCount = BigInt((freshPool.positionCount as BN).toString());

    // Deposit amounts: $5M per asset per tick
    const amounts: BN[] = [];
    for (let i = 0; i < 8; i++) {
      if (i < nAssets) {
        amounts.push(new BN(LIQUIDITY_PER_ASSET.toString()));
      } else {
        amounts.push(new BN(0));
      }
    }

    for (const tick of createdTicks) {
      const [positionPda] = derivePositionPda(
        poolPda,
        deployer.publicKey,
        positionCount
      );

      const amtM = (Number(LIQUIDITY_PER_ASSET) / 1e6 / 1e6).toFixed(1);
      console.log(`  Adding $${amtM}M/asset to tick[${tick.index}]...`);

      // remaining_accounts: [vaults(rw), atas(rw), tick(rw)]
      const remainingAccounts = [
        ...vaultKeys.map((pk) => ({
          pubkey: pk,
          isSigner: false,
          isWritable: true,
        })),
        ...ataAddresses.map((pk) => ({
          pubkey: pk,
          isSigner: false,
          isWritable: true,
        })),
        {
          pubkey: tick.pda,
          isSigner: false,
          isWritable: true,
        },
      ];

      try {
        const tx = await program.methods
          .addLiquidity({ amounts })
          .accounts({
            provider: deployer.publicKey,
            pool: poolPda,
            position: positionPda,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .remainingAccounts(remainingAccounts)
          .signers([deployer])
          .rpc();

        console.log(`    ✓ tx: ${tx}`);
        positionCount += BigInt(1);
      } catch (err) {
        console.error(`    ✗ Failed: ${err}`);
        // Continue with next tick
      }
    }
  }

  // 8. Summary
  console.log(`\n${"═".repeat(80)}`);
  console.log("=== Demo Tick Summary ===\n");

  for (const tick of createdTicks) {
    const kFloat = fpToFloat(tick.k);
    console.log(`  tick[${tick.index}]: ${tick.pda.toBase58()}`);
    console.log(`    k = ${kFloat.toFixed(2)}`);
  }

  // Update devnet-config with tick info
  const config = JSON.parse(fs.readFileSync(CONFIG_PATH, "utf-8"));
  config.ticks = createdTicks.map((t) => ({
    index: t.index,
    address: t.pda.toBase58(),
    kRaw: t.k.toString(),
    label: DEMO_TICK_LABELS[createdTicks.indexOf(t)],
  }));
  config.generatedAt = new Date().toISOString();
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2) + "\n");

  console.log(`\nConfig updated: ${CONFIG_PATH}`);
  console.log("\n=== Done ===");
}

main().catch((err) => {
  console.error("\nFailed:", err);
  process.exit(1);
});
