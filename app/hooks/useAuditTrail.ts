"use client";

/**
 * Hook: fetch all SettlementState accounts for the current pool.
 *
 * Uses getProgramAccounts with discriminator + pool memcmp filters.
 * Polls every 30 seconds (same cadence as useUserPositions).
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { createSolanaRpc, type Address, getAddressEncoder } from "@solana/kit";
import type { Base64EncodedBytes } from "@solana/rpc-types";
import { PROGRAM_ID, POOL_PDA, DEVNET_CONFIG } from "../lib/devnet-config";
import { decodeAccountData } from "../lib/format-utils";
import {
  deserializeSettlementState,
  SETTLEMENT_DISCRIMINATOR,
  type SettlementRecord,
} from "../lib/settlement-deserializer";

const REFRESH_INTERVAL_MS = 30_000;

interface UseAuditTrailResult {
  settlements: SettlementRecord[];
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function useAuditTrail(): UseAuditTrailResult {
  const [settlements, setSettlements] = useState<SettlementRecord[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const mountedRef = useRef(true);

  const fetchSettlements = useCallback(async () => {
    try {
      const rpc = createSolanaRpc(DEVNET_CONFIG.rpcUrl);
      const encoder = getAddressEncoder();

      const discBase64 = btoa(
        String.fromCharCode(...SETTLEMENT_DISCRIMINATOR),
      );
      const poolBase64 = btoa(
        String.fromCharCode(...encoder.encode(POOL_PDA as Address)),
      );

      const result = await rpc
        .getProgramAccounts(PROGRAM_ID as Address, {
          encoding: "base64",
          filters: [
            { memcmp: { offset: 0n, bytes: discBase64 as Base64EncodedBytes, encoding: "base64" } },
            { memcmp: { offset: 9n, bytes: poolBase64 as Base64EncodedBytes, encoding: "base64" } },
          ],
        })
        .send();

      if (!mountedRef.current) return;

      const records: SettlementRecord[] = [];

      for (const item of result) {
        try {
          const bytes = decodeAccountData(item.account.data);
          const record = deserializeSettlementState(String(item.pubkey), bytes);
          records.push(record);
        } catch {
          // Skip malformed accounts
        }
      }

      records.sort((a, b) => b.executedAt - a.executedAt);

      setSettlements(records);
      setError(null);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      if (mountedRef.current) setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    fetchSettlements();

    const interval = setInterval(fetchSettlements, REFRESH_INTERVAL_MS);
    return () => {
      mountedRef.current = false;
      clearInterval(interval);
    };
  }, [fetchSettlements]);

  return { settlements, isLoading, error, refresh: fetchSettlements };
}
