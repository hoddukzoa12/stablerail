"use client";

/**
 * Hook: compute off-chain swap quote with debounced input.
 *
 * Calls computeSwapQuote() from the stablerail-math SDK whenever
 * pool state or input amount changes. Debounces by 300ms to avoid
 * excessive computation during typing.
 */

import { useState, useEffect, useRef } from "react";
import { Q6464, computeSwapQuote } from "../lib/stablerail-math";
import type { PoolState, SwapQuote } from "../lib/stablerail-math";

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
 */
export function useSwapQuote(
  pool: PoolState | null,
  tokenInIndex: number,
  tokenOutIndex: number,
  amountIn: string,
  decimals: number,
): UseSwapQuoteResult {
  const [quote, setQuote] = useState<SwapQuote | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isComputing, setIsComputing] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

    const parsedAmount = parseFloat(trimmed);
    if (isNaN(parsedAmount) || parsedAmount <= 0) {
      setQuote(null);
      setError(null);
      setIsComputing(false);
      return;
    }

    setIsComputing(true);

    timerRef.current = setTimeout(() => {
      try {
        // Convert human-readable amount to base units then to Q64.64
        const baseUnits = BigInt(
          Math.floor(parsedAmount * 10 ** decimals),
        );
        const amountQ = Q6464.fromTokenAmount(baseUnits, decimals);

        const result = computeSwapQuote(
          pool,
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
  }, [pool, tokenInIndex, tokenOutIndex, amountIn, decimals]);

  return { quote, error, isComputing };
}
