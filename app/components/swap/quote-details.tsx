"use client";

import { useState } from "react";
import { ChevronDown } from "lucide-react";
import type { SwapQuote } from "../../lib/stablerail-math";
import type { TokenInfo } from "../../lib/tokens";

interface QuoteDetailsProps {
  quote: SwapQuote | null;
  tokenIn: TokenInfo;
  tokenOut: TokenInfo;
  slippageBps: number;
}

/** Format basis points to percentage string */
function bpsToPercent(bps: number): string {
  const pct = bps / 100;
  if (pct < 0.01) return "< 0.01%";
  return `${pct.toFixed(2)}%`;
}

/** Get color class for price impact severity */
function impactColor(bps: number): string {
  if (bps <= 10) return "text-success";       // < 0.1%
  if (bps <= 50) return "text-warning";       // < 0.5%
  return "text-error";                         // > 0.5%
}

export function QuoteDetails({
  quote,
  tokenIn,
  tokenOut,
  slippageBps,
}: QuoteDetailsProps) {
  const [expanded, setExpanded] = useState(false);

  if (!quote) return null;

  // Exchange rate: 1 tokenIn = ? tokenOut
  const rate = quote.amountOut.toNumber() / quote.amountIn.toNumber();
  const rateStr = rate.toFixed(6);

  // Fee in human-readable units
  const feeHuman =
    quote.feeAmount.toNumber() /
    (quote.amountIn.toNumber() > 0 ? 1 : 1);
  const feeStr = feeHuman.toFixed(6);

  // Minimum received after slippage
  const minReceived =
    Number(quote.amountOutU64) * (1 - slippageBps / 10000);
  const minReceivedStr = (minReceived / 10 ** tokenOut.decimals).toFixed(
    tokenOut.decimals > 4 ? 4 : tokenOut.decimals,
  );

  return (
    <div className="rounded-xl border border-border-subtle bg-surface-1/50">
      {/* Collapsed summary */}
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center justify-between px-4 py-3 text-sm cursor-pointer"
      >
        <span className="text-text-secondary">
          1 {tokenIn.symbol} ≈ {rateStr} {tokenOut.symbol}
        </span>
        <ChevronDown
          className={`h-4 w-4 text-text-tertiary transition-transform ${
            expanded ? "rotate-180" : ""
          }`}
        />
      </button>

      {/* Expanded details */}
      {expanded && (
        <div className="border-t border-border-subtle px-4 pb-3 pt-2 space-y-2">
          {/* Price Impact */}
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-tertiary">Price Impact</span>
            <span className={impactColor(quote.priceImpactBps)}>
              {bpsToPercent(quote.priceImpactBps)}
            </span>
          </div>

          {/* LP Fee */}
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-tertiary">LP Fee</span>
            <span className="text-text-secondary">
              {feeStr} {tokenIn.symbol}
            </span>
          </div>

          {/* Minimum Received */}
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-tertiary">Min. Received</span>
            <span className="text-text-secondary">
              {minReceivedStr} {tokenOut.symbol}
            </span>
          </div>

          {/* Slippage Tolerance */}
          <div className="flex items-center justify-between text-sm">
            <span className="text-text-tertiary">Slippage Tolerance</span>
            <span className="text-text-secondary">
              {bpsToPercent(slippageBps)}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
