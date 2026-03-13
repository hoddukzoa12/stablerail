"use client";

/**
 * Hook: fetch and deserialize the on-chain PoolState account.
 *
 * Uses useSolanaClient() for direct RPC access, with manual
 * polling every 15 seconds to keep pool state reasonably fresh.
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { useSolanaClient } from "@solana/react-hooks";
import { type Address } from "@solana/kit";
import { POOL_PDA } from "../lib/devnet-config";
import { deserializePoolState } from "../lib/pool-deserializer";
import type { PoolState } from "../lib/stablerail-math";

/** Refresh interval for pool state (15 seconds) */
const REFRESH_INTERVAL_MS = 15_000;

interface UsePoolStateResult {
  pool: PoolState | null;
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function usePoolState(): UsePoolStateResult {
  const client = useSolanaClient();
  const [pool, setPool] = useState<PoolState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const mountedRef = useRef(true);

  const fetchPool = useCallback(async () => {
    try {
      const result = await client.runtime.rpc
        .getAccountInfo(POOL_PDA as Address, { encoding: "base64" })
        .send();

      if (!mountedRef.current) return;

      if (!result.value) {
        setPool(null);
        setError(new Error("Pool account not found"));
        setIsLoading(false);
        return;
      }

      // RPC returns base64-encoded data as [base64String, "base64"]
      const rawData = result.value.data;
      let bytes: Uint8Array;

      if (Array.isArray(rawData) && typeof rawData[0] === "string") {
        const decoded = atob(rawData[0] as string);
        bytes = new Uint8Array(decoded.length);
        for (let i = 0; i < decoded.length; i++) {
          bytes[i] = decoded.charCodeAt(i);
        }
      } else if (rawData instanceof Uint8Array) {
        bytes = rawData;
      } else {
        throw new Error("Unexpected account data format");
      }

      const poolState = deserializePoolState(bytes);
      setPool(poolState);
      setError(null);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      if (mountedRef.current) setIsLoading(false);
    }
  }, [client]);

  // Initial fetch + polling
  useEffect(() => {
    mountedRef.current = true;
    fetchPool();

    const interval = setInterval(fetchPool, REFRESH_INTERVAL_MS);
    return () => {
      mountedRef.current = false;
      clearInterval(interval);
    };
  }, [fetchPool]);

  return { pool, isLoading, error, refresh: fetchPool };
}
