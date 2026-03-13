/**
 * Token configuration for the StableRail 3-asset pool.
 *
 * Centralizes token metadata (symbol, mint, vault, decimals, index)
 * so every component references a single source of truth.
 */

import { MINTS, VAULTS, DEVNET_CONFIG } from "./devnet-config";

export interface TokenInfo {
  /** Display symbol, e.g. "USDC" */
  symbol: string;
  /** Full name, e.g. "USD Coin" */
  name: string;
  /** SPL mint address (devnet) */
  mint: string;
  /** Pool vault address (devnet) */
  vault: string;
  /** Token decimal places (6 for all stablecoins) */
  decimals: number;
  /** Index in the on-chain pool reserves array (0, 1, 2) */
  index: number;
  /** CSS color variable for UI indicators */
  color: string;
  /** Raw hex color for libraries that can't use CSS vars (e.g. recharts) */
  colorHex: string;
  /** Path to token icon SVG */
  icon: string;
}

/**
 * Ordered token list matching on-chain pool reserve indices.
 *
 * Index 0 = USDC, 1 = USDT, 2 = PYUSD
 * (Matches the order used in bootstrap-pool.ts)
 */
export const TOKENS: readonly TokenInfo[] = [
  {
    symbol: "USDC",
    name: "USD Coin",
    mint: MINTS["mock-USDC"],
    vault: VAULTS["mock-USDC"],
    decimals: DEVNET_CONFIG.params.decimals,
    index: 0,
    color: "var(--token-usdc)",
    colorHex: "#2775CA",
    icon: "/tokens/usdc.svg",
  },
  {
    symbol: "USDT",
    name: "Tether USD",
    mint: MINTS["mock-USDT"],
    vault: VAULTS["mock-USDT"],
    decimals: DEVNET_CONFIG.params.decimals,
    index: 1,
    color: "var(--token-usdt)",
    colorHex: "#26A17B",
    icon: "/tokens/usdt.svg",
  },
  {
    symbol: "PYUSD",
    name: "PayPal USD",
    mint: MINTS["mock-PYUSD"],
    vault: VAULTS["mock-PYUSD"],
    decimals: DEVNET_CONFIG.params.decimals,
    index: 2,
    color: "var(--token-pyusd)",
    colorHex: "#0033A0",
    icon: "/tokens/pyusd.svg",
  },
] as const;

/** Lookup by symbol (case-insensitive). Throws if not found. */
export function getTokenBySymbol(symbol: string): TokenInfo {
  const upper = symbol.toUpperCase();
  const token = TOKENS.find((t) => t.symbol === upper);
  if (!token) throw new Error(`Unknown token symbol: ${symbol}`);
  return token;
}

/** Lookup by pool reserve index. Throws if out of bounds. */
export function getTokenByIndex(index: number): TokenInfo {
  const token = TOKENS[index];
  if (!token) throw new Error(`Token index out of bounds: ${index}`);
  return token;
}
