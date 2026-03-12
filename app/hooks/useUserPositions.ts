"use client";

/**
 * Hook: fetch LP positions owned by the connected wallet.
 *
 * Uses getProgramAccounts with memcmp filters:
 *   - Discriminator match at offset 0 (8 bytes)
 *   - Owner match at offset 73 (32 bytes): 8(disc) + 1(bump) + 32(pool) + 32(tick) = 73
 *
 * PositionState layout (Borsh, sequential):
 *   Offset  Field           Type      Size
 *   0       discriminator   [u8; 8]   8
 *   8       bump            u8        1
 *   9       pool            Pubkey    32
 *   41      tick            Pubkey    32
 *   73      owner           Pubkey    32
 *   105     liquidity.raw   i128      16
 *   121     tick_lower.raw  i128      16
 *   137     tick_upper.raw  i128      16
 *   153     fees_earned.raw i128      16
 *   169     created_at      i64       8
 *   177     updated_at      i64       8
 *   185     _reserved       [u8; 64]  64
 *   Total: 249 bytes
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { useWalletConnection } from "@solana/react-hooks";
import { createSolanaRpc, type Address, getAddressEncoder } from "@solana/kit";
import type { Base64EncodedBytes } from "@solana/rpc-types";
import { PROGRAM_ID } from "../lib/devnet-config";
import { q6464ToNumber } from "../lib/format-utils";

/** PositionState account discriminator: sha256("account:PositionState")[..8] */
const POSITION_DISCRIMINATOR = new Uint8Array([154, 47, 151, 70, 8, 128, 206, 231]);

/** Polling interval in ms */
const POLL_INTERVAL = 30_000;

export interface UserPosition {
  /** Position account public key */
  address: string;
  /** Raw liquidity amount (i128) */
  liquidityRaw: bigint;
  /** Liquidity as a display number (lossy) */
  liquidityDisplay: number;
  /** Fees earned raw (i128) */
  feesEarnedRaw: bigint;
  /** Creation timestamp (unix seconds) */
  createdAt: number;
}

/**
 * Read a little-endian i128 from a DataView as a BigInt.
 */
function readI128LE(view: DataView, offset: number): bigint {
  const lo = view.getBigUint64(offset, true);
  const hi = view.getBigInt64(offset + 8, true);
  return (hi << 64n) | lo;
}

function parsePositionAccount(address: string, data: Uint8Array): UserPosition {
  const view = new DataView(data.buffer, data.byteOffset, data.byteLength);

  // liquidity at offset 105, i128 LE
  const liquidityRaw = readI128LE(view, 105);
  const liquidityDisplay = q6464ToNumber(liquidityRaw);

  // fees_earned at offset 153, i128 LE
  const feesEarnedRaw = readI128LE(view, 153);

  // created_at at offset 169, i64 LE
  const createdAt = Number(view.getBigInt64(169, true));

  return {
    address,
    liquidityRaw,
    liquidityDisplay,
    feesEarnedRaw,
    createdAt,
  };
}

export function useUserPositions() {
  const { wallet } = useWalletConnection();
  const [positions, setPositions] = useState<UserPosition[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchPositions = useCallback(async () => {
    const userAddress = wallet?.account.address;
    if (!userAddress) {
      setPositions([]);
      return;
    }

    setIsLoading(true);

    try {
      const rpc = createSolanaRpc("https://api.devnet.solana.com");
      const encoder = getAddressEncoder();

      // Encode filters for getProgramAccounts
      const discriminatorBase64 = btoa(
        String.fromCharCode(...POSITION_DISCRIMINATOR),
      );
      const ownerBytes = encoder.encode(userAddress);
      const ownerBase64 = btoa(String.fromCharCode(...ownerBytes));

      const accounts = await rpc
        .getProgramAccounts(PROGRAM_ID as Address, {
          encoding: "base64",
          filters: [
            { memcmp: { offset: 0n, bytes: discriminatorBase64 as Base64EncodedBytes, encoding: "base64" } },
            { memcmp: { offset: 73n, bytes: ownerBase64 as Base64EncodedBytes, encoding: "base64" } },
          ],
        })
        .send();

      const parsed: UserPosition[] = [];
      for (const acct of accounts) {
        const rawData = acct.account.data;
        // base64-encoded data: [base64string, "base64"]
        const b64 =
          typeof rawData === "string"
            ? rawData
            : Array.isArray(rawData)
              ? (rawData as string[])[0]
              : "";
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        parsed.push(parsePositionAccount(String(acct.pubkey), bytes));
      }

      // Sort by creation time descending (newest first)
      parsed.sort((a, b) => b.createdAt - a.createdAt);
      setPositions(parsed);
    } catch (err) {
      console.error("Failed to fetch user positions:", err);
    } finally {
      setIsLoading(false);
    }
  }, [wallet]);

  // Poll every 30s
  useEffect(() => {
    fetchPositions();

    intervalRef.current = setInterval(fetchPositions, POLL_INTERVAL);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchPositions]);

  return { positions, isLoading, refresh: fetchPositions };
}
