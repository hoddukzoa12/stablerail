import { NextRequest, NextResponse } from "next/server";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import {
  createMintToInstruction,
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  getAccount,
} from "@solana/spl-token";

const RPC_URL = "https://api.devnet.solana.com";

const MINT_ADDRESSES: Record<string, string> = {
  "mock-USDC": "FNbLLK2RcFAgmCu8ssHD4smu9udd9E5d7Cj9VmjnPAzc",
  "mock-USDT": "7pXn9qaBoE9JsQ6DbKGNRfXPRvqd8oi1YfyYtRRgxmwV",
  "mock-PYUSD": "6E15qXfBKzudpA9vSdvQAZRZKEB9zbCfHpKnkDGWmwBD",
};

const TOKEN_SYMBOLS = Object.keys(MINT_ADDRESSES).map((k) => k.replace("mock-", ""));

/** 10,000 tokens per mint (decimals = 6) */
const AMOUNT_PER_TOKEN = 10_000 * 1_000_000;
const RATE_LIMIT_MS = 60_000;

const rateLimitMap = new Map<string, number>();

function loadDeployerKeypair(): Keypair {
  const raw = process.env.DEPLOYER_KEYPAIR;
  if (!raw) {
    throw new Error("DEPLOYER_KEYPAIR environment variable not set");
  }
  return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(raw)));
}

function isValidSolanaAddress(addr: string): boolean {
  try {
    new PublicKey(addr);
    return true;
  } catch {
    return false;
  }
}

function jsonError(error: string, status: number): NextResponse {
  return NextResponse.json({ error }, { status });
}

export async function POST(request: NextRequest): Promise<NextResponse> {
  try {
    const { wallet } = await request.json();

    if (!wallet || typeof wallet !== "string") {
      return jsonError("Missing or invalid wallet address", 400);
    }

    if (!isValidSolanaAddress(wallet)) {
      return jsonError("Invalid Solana address", 400);
    }

    const now = Date.now();
    const lastRequest = rateLimitMap.get(wallet);
    if (lastRequest && now - lastRequest < RATE_LIMIT_MS) {
      const waitSec = Math.ceil((RATE_LIMIT_MS - (now - lastRequest)) / 1000);
      return jsonError(`Rate limited. Try again in ${waitSec} seconds.`, 429);
    }

    const deployer = loadDeployerKeypair();
    const connection = new Connection(RPC_URL, "confirmed");
    const walletPubkey = new PublicKey(wallet);

    const transaction = new Transaction();

    for (const mintAddress of Object.values(MINT_ADDRESSES)) {
      const mint = new PublicKey(mintAddress);
      const ata = await getAssociatedTokenAddress(mint, walletPubkey);

      // Create ATA if it doesn't exist yet
      try {
        await getAccount(connection, ata);
      } catch {
        transaction.add(
          createAssociatedTokenAccountInstruction(deployer.publicKey, ata, walletPubkey, mint),
        );
      }

      transaction.add(
        createMintToInstruction(mint, ata, deployer.publicKey, AMOUNT_PER_TOKEN),
      );
    }

    const signature = await sendAndConfirmTransaction(
      connection,
      transaction,
      [deployer],
      { commitment: "confirmed" },
    );

    rateLimitMap.set(wallet, now);

    return NextResponse.json({
      success: true,
      signature,
      amount: AMOUNT_PER_TOKEN / 1_000_000,
      tokens: TOKEN_SYMBOLS,
    });
  } catch (err) {
    console.error("Faucet error:", err);
    return jsonError(
      err instanceof Error ? err.message : "Internal server error",
      500,
    );
  }
}
