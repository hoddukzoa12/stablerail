"use client";

import { useState, useCallback } from "react";
import { useWalletConnection } from "@solana/react-hooks";

interface FaucetResponse {
  success: boolean;
  signature: string;
  amount: number;
  tokens: string[];
  error?: string;
}

export interface UseFaucetResult {
  requestTokens: () => Promise<string | null>;
  isLoading: boolean;
  signature: string | null;
  error: string | null;
}

export function useFaucet(): UseFaucetResult {
  const { wallet } = useWalletConnection();
  const [isLoading, setIsLoading] = useState(false);
  const [signature, setSignature] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const requestTokens = useCallback(async (): Promise<string | null> => {
    const walletAddress = wallet?.account.address;
    if (!walletAddress) {
      setError("Wallet not connected");
      return null;
    }

    setIsLoading(true);
    setError(null);
    setSignature(null);

    try {
      const res = await fetch("/api/faucet", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ wallet: walletAddress.toString() }),
      });

      const data: FaucetResponse = await res.json();

      if (!res.ok || !data.success) {
        setError(data.error || `Request failed (${res.status})`);
        return null;
      }

      setSignature(data.signature);
      return data.signature;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Network error");
      return null;
    } finally {
      setIsLoading(false);
    }
  }, [wallet]);

  return { requestTokens, isLoading, signature, error };
}
