"use client";

/**
 * Generic hook for fetching and deserializing a single on-chain account.
 *
 * Encapsulates the shared polling pattern used by usePoolState,
 * usePolicy, and useAllowlist: getAccountInfo + base64 decode +
 * deserialize + interval refresh.
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { useSolanaClient } from "@solana/react-hooks";
import { type Address } from "@solana/kit";
import { decodeAccountData } from "../lib/format-utils";

interface UseAccountDataResult<T> {
  data: T | null;
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function useAccountData<T>(
  address: string,
  deserialize: (bytes: Uint8Array) => T,
  intervalMs: number,
  accountLabel: string,
): UseAccountDataResult<T> {
  const client = useSolanaClient();
  const [data, setData] = useState<T | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const mountedRef = useRef(true);

  const fetch = useCallback(async () => {
    try {
      const result = await client.runtime.rpc
        .getAccountInfo(address as Address, { encoding: "base64" })
        .send();

      if (!mountedRef.current) return;

      if (!result.value) {
        setData(null);
        setError(new Error(`${accountLabel} account not found`));
        setIsLoading(false);
        return;
      }

      const bytes = decodeAccountData(result.value.data);
      const deserialized = deserialize(bytes);
      setData(deserialized);
      setError(null);
    } catch (err) {
      if (!mountedRef.current) return;
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      if (mountedRef.current) setIsLoading(false);
    }
  }, [client, address, deserialize, accountLabel]);

  useEffect(() => {
    mountedRef.current = true;
    fetch();

    const interval = setInterval(fetch, intervalMs);
    return () => {
      mountedRef.current = false;
      clearInterval(interval);
    };
  }, [fetch, intervalMs]);

  return { data, isLoading, error, refresh: fetch };
}
