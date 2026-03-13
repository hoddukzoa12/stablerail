"use client";

import { POLICY_PDA } from "../lib/devnet-config";
import {
  deserializePolicyState,
  type PolicyStateData,
} from "../lib/settlement-deserializer";
import { useAccountData } from "./useAccountData";

const REFRESH_INTERVAL_MS = 15_000;

interface UsePolicyResult {
  policy: PolicyStateData | null;
  isLoading: boolean;
  error: Error | null;
  refresh: () => void;
}

export function usePolicy(): UsePolicyResult {
  const { data, isLoading, error, refresh } = useAccountData(
    POLICY_PDA,
    deserializePolicyState,
    REFRESH_INTERVAL_MS,
    "Policy",
  );

  return { policy: data, isLoading, error, refresh };
}
