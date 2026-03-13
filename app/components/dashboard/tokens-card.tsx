"use client";

import { Card } from "../ui/card";
import { TOKENS } from "../../lib/tokens";
import { formatBalance, formatUsd } from "../../lib/format-utils";

interface TokensCardProps {
  balances: Record<string, bigint>;
}

export function TokensCard({ balances }: TokensCardProps) {
  const totalUsd = TOKENS.reduce((sum, token) => {
    const bal = balances[token.symbol] ?? 0n;
    return sum + Number(bal) / 10 ** token.decimals;
  }, 0);

  return (
    <Card variant="glass" className="p-5">
      <div className="mb-4 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-primary">Tokens</h3>
        <span className="font-mono text-sm font-semibold text-text-primary">
          {formatUsd(totalUsd)}
        </span>
      </div>

      <div className="space-y-3">
        {TOKENS.map((token) => {
          const bal = balances[token.symbol] ?? 0n;
          const balNum = Number(bal) / 10 ** token.decimals;
          return (
            <div
              key={token.symbol}
              className="flex items-center justify-between rounded-lg bg-surface-2 px-3 py-2.5"
            >
              <div className="flex items-center gap-2.5">
                <div
                  className="flex h-8 w-8 items-center justify-center rounded-full"
                  style={{ backgroundColor: token.colorHex }}
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
                  {formatUsd(balNum)}
                </p>
              </div>
            </div>
          );
        })}
      </div>
    </Card>
  );
}
