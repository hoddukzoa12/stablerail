"use client";

import { Card } from "../ui/card";
import { TOKENS } from "../../lib/tokens";

interface TokensCardProps {
  balances: Record<string, bigint>;
}

const TOKEN_COLORS: Record<string, string> = {
  USDC: "#2775CA",
  USDT: "#26A17B",
  PYUSD: "#0033A0",
};

/** Floor truncation: never show more than actual balance */
function formatBalance(baseUnits: bigint, decimals: number): string {
  const whole = Number(baseUnits) / 10 ** decimals;
  if (whole === 0) return "—";
  const floored = Math.floor(whole * 100) / 100;
  return floored.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

function formatUsd(baseUnits: bigint, decimals: number): string {
  const value = Number(baseUnits) / 10 ** decimals;
  if (value === 0) return "$0.00";
  const floored = Math.floor(value * 100) / 100;
  if (floored >= 1_000_000) return `$${Math.floor(floored / 10_000) / 100}M`;
  if (floored >= 1_000) return `$${Math.floor(floored / 10) / 100}K`;
  return `$${floored.toFixed(2)}`;
}

export function TokensCard({ balances }: TokensCardProps) {
  // Total USD value (stablecoins ≈ $1 each)
  const totalUsd = TOKENS.reduce((sum, token) => {
    const bal = balances[token.symbol] ?? 0n;
    return sum + Number(bal) / 10 ** token.decimals;
  }, 0);

  return (
    <Card variant="glass" className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-primary">Tokens</h3>
        <span className="font-mono text-sm font-semibold text-text-primary">
          {totalUsd === 0
            ? "$0.00"
            : totalUsd >= 1_000
              ? `$${(Math.floor(totalUsd / 10) / 100).toFixed(2)}K`
              : `$${(Math.floor(totalUsd * 100) / 100).toFixed(2)}`}
        </span>
      </div>

      <div className="space-y-3">
        {TOKENS.map((token) => {
          const bal = balances[token.symbol] ?? 0n;
          return (
            <div
              key={token.symbol}
              className="flex items-center justify-between rounded-lg bg-surface-2 px-3 py-2.5"
            >
              <div className="flex items-center gap-2.5">
                <div
                  className="flex h-8 w-8 items-center justify-center rounded-full"
                  style={{ backgroundColor: TOKEN_COLORS[token.symbol] }}
                >
                  <span className="text-[10px] font-bold text-white">
                    {token.symbol.charAt(0)}
                  </span>
                </div>
                <div>
                  <p className="text-sm font-medium text-text-primary">
                    {token.symbol}
                  </p>
                  <p className="text-xs text-text-tertiary">{token.name}</p>
                </div>
              </div>

              <div className="text-right">
                <p className="font-mono text-sm font-medium text-text-primary">
                  {formatBalance(bal, token.decimals)}
                </p>
                <p className="font-mono text-xs text-text-tertiary">
                  {formatUsd(bal, token.decimals)}
                </p>
              </div>
            </div>
          );
        })}
      </div>
    </Card>
  );
}
