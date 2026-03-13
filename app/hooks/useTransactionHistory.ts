"use client";

/**
 * Hook: fetch on-chain transaction history for the connected wallet.
 *
 * Approach:
 *   1. getSignaturesForAddress(walletAddress, { limit: 20 })
 *   2. For each signature → getTransaction(sig) with maxSupportedTransactionVersion
 *   3. Match instruction program ID === PROGRAM_ID
 *   4. Match first 8 bytes of instruction data → tx type discriminator
 *
 * Instruction discriminators (sha256("global:<fn_name>")[..8]):
 *   execute_swap:       [56, 182, 124, 215, 155, 140, 157, 102]
 *   add_liquidity:      [181, 157, 89, 67, 143, 182, 52, 72]
 *   remove_liquidity:   [80, 85, 209, 72, 24, 206, 177, 108]
 *   execute_settlement: [237, 120, 82, 62, 224, 193, 147, 137]
 *
 * Polls every 30 seconds (same cadence as useUserPositions).
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { useWalletConnection } from "@solana/react-hooks";
import { createSolanaRpc, type Address } from "@solana/kit";
import { PROGRAM_ID } from "../lib/devnet-config";

/** Polling interval in ms */
const POLL_INTERVAL = 30_000;

/** Max signatures to fetch per request */
const SIGNATURE_LIMIT = 20;

/** Known instruction discriminators (first 8 bytes) */
const DISCRIMINATORS: Record<string, TransactionType> = {};

type TransactionType =
  | "Swap"
  | "Add Liquidity"
  | "Remove Liquidity"
  | "Settlement"
  | "Unknown";

// Build a hex-key lookup table for fast matching
const DISC_ENTRIES: Array<[Uint8Array, TransactionType]> = [
  [new Uint8Array([56, 182, 124, 215, 155, 140, 157, 102]), "Swap"],
  [new Uint8Array([181, 157, 89, 67, 143, 182, 52, 72]), "Add Liquidity"],
  [new Uint8Array([80, 85, 209, 72, 24, 206, 177, 108]), "Remove Liquidity"],
  [new Uint8Array([237, 120, 82, 62, 224, 193, 147, 137]), "Settlement"],
];

function uint8ToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// Pre-compute hex keys for O(1) lookup
for (const [disc, type] of DISC_ENTRIES) {
  DISCRIMINATORS[uint8ToHex(disc)] = type;
}

export interface Transaction {
  /** Transaction signature (base58) */
  signature: string;
  /** Detected instruction type */
  type: TransactionType;
  /** Block time in unix seconds */
  timestamp: number;
  /** Whether the transaction succeeded */
  status: "success" | "failed";
}

export function useTransactionHistory() {
  const { wallet } = useWalletConnection();
  const [transactions, setTransactions] = useState<Transaction[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchTransactions = useCallback(async () => {
    const userAddress = wallet?.account.address;
    if (!userAddress) {
      setTransactions([]);
      return;
    }

    setIsLoading(true);

    try {
      const rpc = createSolanaRpc("https://api.devnet.solana.com");

      // Step 1: Get recent signatures for the wallet
      const signaturesResult = await rpc
        .getSignaturesForAddress(userAddress as Address, {
          limit: SIGNATURE_LIMIT,
        })
        .send();

      if (!signaturesResult || signaturesResult.length === 0) {
        setTransactions([]);
        return;
      }

      // Step 2: Fetch each transaction and classify
      const txResults: Transaction[] = [];

      for (const sigInfo of signaturesResult) {
        try {
          const txResult = await rpc
            .getTransaction(sigInfo.signature, {
              encoding: "json",
              maxSupportedTransactionVersion: 0,
            })
            .send();

          if (!txResult) continue;

          // Step 3: Find instructions targeting our program
          const message = txResult.transaction.message;
          const accountKeys = message.accountKeys;

          let txType: TransactionType = "Unknown";
          let isOurProgram = false;

          // Check compiled instructions
          for (const ix of message.instructions) {
            const programIdx =
              typeof ix.programIdIndex === "number"
                ? ix.programIdIndex
                : Number(ix.programIdIndex);
            const programId = accountKeys[programIdx];

            if (String(programId) === PROGRAM_ID) {
              isOurProgram = true;
              // ix.data is base58-encoded in JSON format; decode to match discriminator
              if (ix.data) {
                txType = matchDiscriminatorBase58(String(ix.data));
              }
              break;
            }
          }

          // Also check inner instructions (CPI calls via our program)
          if (!isOurProgram && txResult.meta?.innerInstructions) {
            for (const inner of txResult.meta.innerInstructions) {
              for (const ix of inner.instructions) {
                const programIdx =
                  typeof ix.programIdIndex === "number"
                    ? ix.programIdIndex
                    : Number(ix.programIdIndex);
                const programId = accountKeys[programIdx];
                if (programId && String(programId) === PROGRAM_ID) {
                  isOurProgram = true;
                  if ((ix as { data?: string }).data) {
                    txType = matchDiscriminatorBase58(
                      String((ix as { data?: string }).data),
                    );
                  }
                  break;
                }
              }
              if (isOurProgram) break;
            }
          }

          if (!isOurProgram) continue;

          txResults.push({
            signature: String(sigInfo.signature),
            type: txType,
            timestamp: txResult.blockTime ? Number(txResult.blockTime) : 0,
            status: sigInfo.err ? "failed" : "success",
          });
        } catch {
          // Skip individual tx fetch errors (e.g., old finalized tx)
          continue;
        }
      }

      setTransactions(txResults);
    } catch (err) {
      console.error("Failed to fetch transaction history:", err);
    } finally {
      setIsLoading(false);
    }
  }, [wallet]);

  // Poll every 30s
  useEffect(() => {
    fetchTransactions();

    intervalRef.current = setInterval(fetchTransactions, POLL_INTERVAL);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchTransactions]);

  return { transactions, isLoading, refresh: fetchTransactions };
}

/**
 * Decode base58-encoded instruction data and match against discriminators.
 * Solana JSON encoding returns instruction data as base58 strings.
 */
function matchDiscriminatorBase58(dataBase58: string): TransactionType {
  try {
    const bytes = base58Decode(dataBase58);
    if (bytes.length < 8) return "Unknown";
    const hex = uint8ToHex(bytes.slice(0, 8));
    return DISCRIMINATORS[hex] ?? "Unknown";
  } catch {
    return "Unknown";
  }
}

/**
 * Minimal base58 decoder (Bitcoin alphabet).
 * Avoids adding a dependency for this single use case.
 */
const BASE58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58Decode(str: string): Uint8Array {
  const bytes: number[] = [0];
  for (const char of str) {
    const idx = BASE58_ALPHABET.indexOf(char);
    if (idx < 0) throw new Error(`Invalid base58 char: ${char}`);
    let carry = idx;
    for (let j = 0; j < bytes.length; j++) {
      carry += bytes[j] * 58;
      bytes[j] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      bytes.push(carry & 0xff);
      carry >>= 8;
    }
  }
  // Leading zeros
  for (const char of str) {
    if (char !== "1") break;
    bytes.push(0);
  }
  return new Uint8Array(bytes.reverse());
}
