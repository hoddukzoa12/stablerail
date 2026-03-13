"use client";

import { POOL_PDA } from "../lib/devnet-config";
import { deserializePoolState } from "../lib/pool-deserializer";
import type { PoolState } from "../lib/stablerail-math";
import { useAccountData } from "./useAccountData";

const REFRESH_INTERVAL_MS = 15_000;

interface UsePoolStateResult {
  pool: PoolState | null;
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function usePoolState(): UsePoolStateResult {
  const { data, isLoading, error, refresh } = useAccountData(
    POOL_PDA,
    deserializePoolState,
    REFRESH_INTERVAL_MS,
    "Pool",
  );

  return { pool: data, isLoading, error, refresh };
}
