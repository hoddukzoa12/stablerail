"use client";

import { useCallback } from "react";
import { ALLOWLIST_PDA } from "../lib/devnet-config";
import {
  deserializeAllowlistState,
  type AllowlistStateData,
} from "../lib/settlement-deserializer";
import { useAccountData } from "./useAccountData";

const REFRESH_INTERVAL_MS = 15_000;

interface UseAllowlistResult {
  allowlist: AllowlistStateData | null;
  addresses: string[];
  isAllowed: (address: string) => boolean;
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function useAllowlist(): UseAllowlistResult {
  const { data, isLoading, error, refresh } = useAccountData(
    ALLOWLIST_PDA,
    deserializeAllowlistState,
    REFRESH_INTERVAL_MS,
    "Allowlist",
  );

  const addresses = data?.addresses ?? [];
  const isAllowed = useCallback(
    (address: string) => addresses.includes(address),
    [addresses],
  );

  return { allowlist: data, addresses, isAllowed, isLoading, error, refresh };
}
