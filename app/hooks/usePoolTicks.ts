"use client";

/**
 * Hook: fetch all TickState accounts belonging to the pool.
 *
 * Uses getProgramAccounts with memcmp filters:
 *   - Discriminator match at offset 0 (8 bytes)
 *   - Pool pubkey match at offset 9 (32 bytes): 8(disc) + 1(bump) = 9
 *
 * TickState layout reference: see tick-deserializer.ts
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { createSolanaRpc, type Address, getAddressEncoder } from "@solana/kit";
import type { Base64EncodedBytes } from "@solana/rpc-types";
import { PROGRAM_ID, POOL_PDA } from "../lib/devnet-config";
import {
  deserializeTickState,
  TICK_DISCRIMINATOR,
  type TickInfo,
} from "../lib/tick-deserializer";

/** Polling interval in ms */
const POLL_INTERVAL = 30_000;

export function usePoolTicks(nAssets: number = 3) {
  const [ticks, setTicks] = useState<TickInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchTicks = useCallback(async () => {
    setIsLoading(true);

    try {
      const rpc = createSolanaRpc("https://api.devnet.solana.com");
      const encoder = getAddressEncoder();

      // Encode filters for getProgramAccounts
      const discriminatorBase64 = btoa(
        String.fromCharCode(...TICK_DISCRIMINATOR),
      );
      const poolBytes = encoder.encode(POOL_PDA as Address);
      const poolBase64 = btoa(String.fromCharCode(...poolBytes));

      const accounts = await rpc
        .getProgramAccounts(PROGRAM_ID as Address, {
          encoding: "base64",
          filters: [
            {
              memcmp: {
                offset: 0n,
                bytes: discriminatorBase64 as Base64EncodedBytes,
                encoding: "base64",
              },
            },
            {
              memcmp: {
                offset: 9n,
                bytes: poolBase64 as Base64EncodedBytes,
                encoding: "base64",
              },
            },
          ],
        })
        .send();

      const parsed: TickInfo[] = [];
      for (const acct of accounts) {
        const rawData = acct.account.data;
        const b64 =
          typeof rawData === "string"
            ? rawData
            : Array.isArray(rawData)
              ? (rawData as string[])[0]
              : "";
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        parsed.push(
          deserializeTickState(String(acct.pubkey), bytes, nAssets),
        );
      }

      // Deduplicate ticks with the same k value — keep the one with highest liquidity
      const byK = new Map<string, TickInfo>();
      for (const t of parsed) {
        const key = t.kRaw.toString();
        const existing = byK.get(key);
        if (!existing || t.liquidityRaw > existing.liquidityRaw) {
          byK.set(key, t);
        }
      }
      const deduped = Array.from(byK.values());

      // Sort by k value ascending
      deduped.sort((a, b) => a.kDisplay - b.kDisplay);
      setTicks(deduped);
      setError(null);
    } catch (err) {
      console.error("Failed to fetch pool ticks:", err);
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setIsLoading(false);
    }
  }, [nAssets]);

  useEffect(() => {
    fetchTicks();

    intervalRef.current = setInterval(fetchTicks, POLL_INTERVAL);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchTicks]);

  return { ticks, isLoading, error, refresh: fetchTicks };
}
