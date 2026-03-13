"use client";

/**
 * Hook: fetch SPL token balances for all pool tokens.
 *
 * Returns a map of token symbol → balance in base units (bigint).
 * Uses useSplToken() for each of the 3 pool tokens.
 */

import { useSplToken, useWalletConnection } from "@solana/react-hooks";
import { TOKENS } from "../lib/tokens";

interface TokenBalances {
  /** Balance per token symbol in base units (e.g. 1_000_000n = 1 USDC) */
  balances: Record<string, bigint>;
  /** True while any balance is still loading */
  isLoading: boolean;
  /** Refresh all balances */
  refresh: () => void;
}

export function useTokenBalances(): TokenBalances {
  const { status } = useWalletConnection();
  const isConnected = status === "connected";

  // Hook calls must be unconditional (Rules of Hooks)
  const usdc = useSplToken(TOKENS[0].mint);
  const usdt = useSplToken(TOKENS[1].mint);
  const pyusd = useSplToken(TOKENS[2].mint);

  const tokens = [usdc, usdt, pyusd];

  const balances: Record<string, bigint> = {};
  let isLoading = false;

  for (let i = 0; i < TOKENS.length; i++) {
    const { balance, status: tokenStatus } = tokens[i];
    if (tokenStatus === "loading") isLoading = true;

    if (!isConnected || !balance) {
      balances[TOKENS[i].symbol] = 0n;
    } else {
      // balance.amount is the raw token amount as a bigint
      balances[TOKENS[i].symbol] =
        typeof balance.amount === "bigint"
          ? balance.amount
          : BigInt(String(balance.amount ?? 0));
    }
  }

  const refresh = () => {
    usdc.refresh();
    usdt.refresh();
    pyusd.refresh();
  };

  return { balances, isLoading, refresh };
}
