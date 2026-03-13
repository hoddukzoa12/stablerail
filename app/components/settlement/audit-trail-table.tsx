"use client";

import { Card } from "../ui/card";
import { Badge } from "../ui/badge";
import type { BadgeVariant } from "../ui/badge";
import { truncateAddress, explorerUrl, formatAmount } from "../../lib/format-utils";
import { getTokenByIndex } from "../../lib/tokens";
import type { SettlementRecord } from "../../lib/settlement-deserializer";

interface AuditTrailTableProps {
  settlements: SettlementRecord[];
  isLoading: boolean;
}

const STATUS_VARIANT: Record<string, BadgeVariant> = {
  Executed: "success",
  Failed: "error",
};

function formatTimestamp(ts: number): string {
  if (ts === 0) return "\u2014";
  return new Date(ts * 1000).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

export function AuditTrailTable({ settlements, isLoading }: AuditTrailTableProps) {
  return (
    <Card variant="glass">
      <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
        Audit Trail
      </h3>

      <div className="mt-4 overflow-x-auto">
        {isLoading ? (
          <div className="flex items-center justify-center py-8">
            <div className="h-6 w-6 animate-spin rounded-full border-2 border-brand-primary border-t-transparent" />
          </div>
        ) : settlements.length === 0 ? (
          <p className="py-8 text-center text-sm text-text-tertiary">
            No settlements recorded yet
          </p>
        ) : (
          <table className="w-full">
            <thead>
              <tr className="border-b border-border-subtle text-left text-xs font-medium uppercase tracking-wider text-text-tertiary">
                <th className="pb-2 pr-4">Time</th>
                <th className="pb-2 pr-4">Executor</th>
                <th className="pb-2 pr-4">Pair</th>
                <th className="pb-2 pr-4 text-right">Amount In</th>
                <th className="pb-2 pr-4 text-right">Amount Out</th>
                <th className="pb-2 text-center">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle">
              {settlements.map((s) => {
                const tokenIn = getTokenByIndex(s.tokenInIndex);
                const tokenOut = getTokenByIndex(s.tokenOutIndex);

                return (
                  <tr key={s.address} className="text-sm">
                    <td className="py-2.5 pr-4 text-text-secondary">
                      {formatTimestamp(s.executedAt)}
                    </td>
                    <td className="py-2.5 pr-4">
                      <a
                        href={explorerUrl("address", s.executor)}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="font-mono text-brand-primary hover:underline"
                      >
                        {truncateAddress(s.executor)}
                      </a>
                    </td>
                    <td className="py-2.5 pr-4 text-text-primary">
                      {tokenIn.symbol} &rarr; {tokenOut.symbol}
                    </td>
                    <td className="py-2.5 pr-4 text-right font-mono text-text-primary">
                      {formatAmount(s.amountIn)}
                    </td>
                    <td className="py-2.5 pr-4 text-right font-mono text-text-primary">
                      {formatAmount(s.amountOut)}
                    </td>
                    <td className="py-2.5 text-center">
                      <Badge variant={STATUS_VARIANT[s.status] ?? "warning"}>
                        {s.status}
                      </Badge>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>
    </Card>
  );
}
