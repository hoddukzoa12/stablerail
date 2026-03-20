"use client";

/**
 * Hook: fetch all KycEntryState accounts for the current policy.
 *
 * Uses getProgramAccounts with memcmp filters:
 *   - Discriminator match at offset 0 (8 bytes)
 *   - Policy pubkey match at offset 9 (32 bytes): 8(disc) + 1(bump) = 9
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { createSolanaRpc, type Address, getAddressEncoder } from "@solana/kit";
import type { Base64EncodedBytes } from "@solana/rpc-types";
import { PROGRAM_ID, POLICY_PDA } from "../lib/devnet-config";
import {
  deserializeKycEntryState,
  KYC_ENTRY_DISCRIMINATOR,
  type KycEntryData,
} from "../lib/settlement-deserializer";

const POLL_INTERVAL = 30_000;

export function useKycEntries() {
  const [entries, setEntries] = useState<KycEntryData[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const fetchEntries = useCallback(async () => {
    setIsLoading(true);

    try {
      const rpc = createSolanaRpc("https://api.devnet.solana.com");
      const encoder = getAddressEncoder();

      const discriminatorBase64 = btoa(
        String.fromCharCode(...KYC_ENTRY_DISCRIMINATOR),
      );
      const policyBytes = encoder.encode(POLICY_PDA as Address);
      const policyBase64 = btoa(String.fromCharCode(...policyBytes));

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
                bytes: policyBase64 as Base64EncodedBytes,
                encoding: "base64",
              },
            },
          ],
        })
        .send();

      const parsed: KycEntryData[] = [];
      for (const acct of accounts) {
        const rawData = acct.account.data;
        if (Array.isArray(rawData) && rawData.length >= 1) {
          const base64Str = rawData[0] as string;
          const bytes = Uint8Array.from(atob(base64Str), (c) => c.charCodeAt(0));
          try {
            parsed.push(deserializeKycEntryState(bytes));
          } catch {
            // Skip malformed entries
          }
        }
      }

      setEntries(parsed);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchEntries();
    intervalRef.current = setInterval(fetchEntries, POLL_INTERVAL);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [fetchEntries]);

  return { entries, isLoading, error, refresh: fetchEntries };
}
