/**
 * Shared formatting utilities for the StableRail frontend.
 *
 * Centralizes Q64.64 conversion, USD formatting, balance display,
 * address truncation, and Solana Explorer URL generation.
 */

// ── Q64.64 Conversion ──

const Q64 = 1n << 64n;

/** Convert Q64.64 fixed-point value to a JavaScript number (lossy). */
export function q6464ToNumber(raw: bigint): number {
  return Number(raw / Q64) + Number(raw % Q64) / Number(Q64);
}

// ── USD Formatting ──

/**
 * Format a number as a compact USD string.
 * Uses floor-based rounding (DeFi standard: never overstate values).
 *
 * @param value  - The numeric value to format
 * @param precision - Decimal places for K/M suffixes (default: 2)
 */
export function formatUsd(value: number, precision = 2): string {
  if (value >= 1_000_000) {
    const floored = Math.floor(value / 10 ** (6 - precision)) / 10 ** precision;
    return `$${floored.toFixed(precision)}M`;
  }
  if (value >= 1_000) {
    const floored = Math.floor(value / 10 ** (3 - precision)) / 10 ** precision;
    return `$${floored.toFixed(precision)}K`;
  }
  return `$${(Math.floor(value * 100) / 100).toFixed(2)}`;
}

// ── Balance Formatting ──

/**
 * Format a token balance with floor truncation to 2 decimals.
 * DeFi standard: never show more than actual balance.
 *
 * @param baseUnits - Raw token amount in base units (e.g. 1_000_000n = 1 USDC)
 * @param decimals  - Token decimal places
 * @param zeroLabel - Label to display when balance is zero (default: "—")
 */
export function formatBalance(
  baseUnits: bigint,
  decimals: number,
  zeroLabel = "—",
): string {
  const whole = Number(baseUnits) / 10 ** decimals;
  if (whole === 0) return zeroLabel;
  const floored = Math.floor(whole * 100) / 100;
  return floored.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

// ── Address Formatting ──

/**
 * Truncate a Solana address/signature for display.
 *
 * @param address - Full base58 address or signature
 * @param prefix  - Number of leading characters to keep (default: 4)
 * @param suffix  - Number of trailing characters to keep (default: 4)
 */
export function truncateAddress(
  address: string,
  prefix = 4,
  suffix = 4,
): string {
  if (address.length <= prefix + suffix + 3) return address;
  return `${address.slice(0, prefix)}...${address.slice(-suffix)}`;
}

// ── Solana Explorer URLs ──

const EXPLORER_BASE = "https://explorer.solana.com";
const CLUSTER = "devnet";

/** Generate a Solana Explorer URL for an address or transaction. */
export function explorerUrl(
  type: "address" | "tx",
  value: string,
): string {
  return `${EXPLORER_BASE}/${type}/${value}?cluster=${CLUSTER}`;
}

// ── Liquidity Helpers ──

/**
 * Compute partial liquidity amount from a percentage.
 * Returns full amount for 100%, otherwise applies BigInt division.
 */
export function computePartialLiquidity(
  total: bigint,
  percent: number,
): bigint {
  if (percent >= 100) return total;
  return (total * BigInt(percent)) / 100n;
}

/** Clamp a percent input string to 1-100 range. */
export function clampPercent(value: string): number {
  return Math.max(1, Math.min(100, Number(value) || 1));
}
