"use client";

/**
 * Hook: compute off-chain swap quote with debounced input.
 *
 * When tick data is provided, uses `computeSwapQuoteWithTicks()` to
 * simulate the on-chain trade segmentation loop (alpha-based crossing
 * detection, delta-to-boundary quadratic solver, tick flipping).
 * Falls back to single-sphere `computeSwapQuote()` when no ticks.
 *
 * Debounces by 300ms to avoid excessive computation during typing.
 */

import { useState, useEffect, useRef, useMemo } from "react";
import {
  Q6464,
  computeSwapQuoteWithTicks,
  parseTokenAmount,
} from "../lib/stablerail-math";
import type {
  PoolState,
  SwapQuote,
  TickData,
} from "../lib/stablerail-math";

/** Debounce delay for amount input changes */
const DEBOUNCE_MS = 300;

interface UseSwapQuoteResult {
  quote: SwapQuote | null;
  error: string | null;
  isComputing: boolean;
}

/**
 * Compute a swap quote reactively as inputs change.
 *
 * @param pool - Current pool state (null while loading)
 * @param tokenInIndex - Index of the input token in the pool
 * @param tokenOutIndex - Index of the output token in the pool
 * @param amountIn - User-entered amount string (e.g. "100.5")
 * @param decimals - Decimal places for the input token
 * @param ticks - Optional tick data for concentrated liquidity routing
 */
export function useSwapQuote(
  pool: PoolState | null,
  tokenInIndex: number,
  tokenOutIndex: number,
  amountIn: string,
  decimals: number,
  ticks?: TickData[],
): UseSwapQuoteResult {
  const [quote, setQuote] = useState<SwapQuote | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isComputing, setIsComputing] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Stable reference for ticks — updated every render so the debounced
  // callback always reads the latest data without being a dependency.
  const ticksRef = useRef<TickData[] | undefined>(ticks);
  ticksRef.current = ticks;

  // Lightweight content fingerprint: encodes tick count + each tick's
  // liquidity. Changes when ticks are added/removed OR when any tick's
  // reserves shift (after swaps or LP actions), triggering quote recomputation
  // without resetting the debounce on every poll cycle.
  const tickFingerprint = useMemo(() => {
    if (!ticks || ticks.length === 0) return "0";
    // Include liquidity AND reserves so quote refreshes after swaps/LP
    // that change per-tick reserves without altering tick count.
    return ticks
      .map((t) => {
        const res = t.reservesRaw.map((r) => r.toString(36)).join("|");
        return `${t.liquidityRaw.toString(36)}:${res}`;
      })
      .join(",");
  }, [ticks]);


  useEffect(() => {
    // Clear previous timer
    if (timerRef.current) clearTimeout(timerRef.current);

    // Reset if inputs are invalid
    const trimmed = amountIn.trim();
    if (!pool || !trimmed || tokenInIndex === tokenOutIndex) {
      setQuote(null);
      setError(null);
      setIsComputing(false);
      return;
    }

    // Validate format without converting through parseFloat (which
    // destroys precision for very small amounts via scientific notation
    // e.g. "0.000001" → 1e-7 → String() → "1e-7" → split(".") breaks).
    if (!/^\d+\.?\d*$/.test(trimmed)) {
      setQuote(null);
      setError(null);
      setIsComputing(false);
      return;
    }

    // Quick positivity check (parseTokenAmount handles the actual conversion)
    const baseUnitsCheck = parseTokenAmount(trimmed, decimals);
    if (baseUnitsCheck <= 0n) {
      setQuote(null);
      setError(null);
      setIsComputing(false);
      return;
    }

    setIsComputing(true);

    timerRef.current = setTimeout(() => {
      try {
        // Convert human-readable amount to base units then to Q64.64.
        // Pass the original trimmed string directly — never round-trip
        // through parseFloat which loses precision for small/large amounts.
        const baseUnits = parseTokenAmount(trimmed, decimals);
        const amountQ = Q6464.fromTokenAmount(baseUnits, decimals);

        // Always use tick-aware path — it handles the tickCount == 0
        // fallback internally and rejects when ticks are missing for
        // pools that have them (prevents misleading single-sphere quotes).
        const currentTicks = ticksRef.current ?? [];
        const result = computeSwapQuoteWithTicks(
          pool,
          currentTicks,
          tokenInIndex,
          tokenOutIndex,
          amountQ,
        );

        setQuote(result);
        setError(null);
      } catch (err) {
        setQuote(null);
        const msg =
          err instanceof Error ? err.message : "Quote computation failed";
        // Provide user-friendly error messages
        if (msg.includes("insufficient liquidity")) {
          setError("Insufficient liquidity for this trade");
        } else if (msg.includes("negative radicand")) {
          setError("Trade size exceeds available liquidity");
        } else {
          setError(msg);
        }
      } finally {
        setIsComputing(false);
      }
    }, DEBOUNCE_MS);

    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
    // `ticks` array reference is excluded (changes every poll cycle).
    // `tickFingerprint` captures both count and content changes so quote
    // recomputes when tick liquidity/reserves shift after swaps or LP actions.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pool, tokenInIndex, tokenOutIndex, amountIn, decimals, tickFingerprint]);

  return { quote, error, isComputing };
}
