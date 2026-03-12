"use client";

import { PieChart, Pie, Cell, ResponsiveContainer, Tooltip } from "recharts";
import type { PoolState } from "../../lib/stablerail-math";
import { TOKENS } from "../../lib/tokens";
import { Card } from "../ui/card";

interface ReserveChartProps {
  pool: PoolState;
}

/** Token colors — resolved from CSS vars for recharts (needs raw hex) */
const TOKEN_COLORS = ["#2775CA", "#26A17B", "#0033A0"];

/** Convert Q64.64 reserve to human-readable token amount.
 *  On-chain reserves are already decimal-normalized via from_token_amount(),
 *  so raw / Q64 gives the whole-token amount directly (e.g., 102 USDC). */
function reserveToDisplay(raw: bigint): number {
  const Q64 = 1n << 64n;
  const intPart = Number(raw / Q64);
  const fracPart = Number(raw % Q64) / Number(Q64);
  return intPart + fracPart;
}

function formatUsd(value: number): string {
  if (value >= 1_000_000) return `$${(value / 1_000_000).toFixed(2)}M`;
  if (value >= 1_000) return `$${(value / 1_000).toFixed(2)}K`;
  return `$${value.toFixed(2)}`;
}

export function ReserveChart({ pool }: ReserveChartProps) {
  const reserves = pool.reserves.map((r) => reserveToDisplay(r.raw));

  const total = reserves.reduce((a, b) => a + b, 0);
  const idealPct = 100 / pool.nAssets;

  const chartData = TOKENS.slice(0, pool.nAssets).map((token, i) => ({
    name: token.symbol,
    value: reserves[i],
    pct: total > 0 ? (reserves[i] / total) * 100 : 100 / pool.nAssets,
    color: TOKEN_COLORS[i],
  }));

  return (
    <Card variant="glass" className="flex h-full flex-col p-5">
      {/* Donut chart + center TVL */}
      <div className="flex flex-1 items-center justify-center py-4">
        <div className="relative h-[280px] w-[280px]">
          <ResponsiveContainer width="100%" height="100%">
            <PieChart>
              <Pie
                data={chartData}
                cx="50%"
                cy="50%"
                innerRadius={85}
                outerRadius={130}
                paddingAngle={2}
                dataKey="value"
                stroke="none"
                animationBegin={0}
                animationDuration={800}
              >
                {chartData.map((entry, i) => (
                  <Cell key={entry.name} fill={entry.color} />
                ))}
              </Pie>
              <Tooltip
                content={({ active, payload }) => {
                  if (!active || !payload?.length) return null;
                  const d = payload[0].payload;
                  return (
                    <div className="rounded-lg bg-surface-1 px-3 py-2 text-xs shadow-lg border border-border-default">
                      <p className="font-medium text-text-primary">{d.name}</p>
                      <p className="font-mono text-text-secondary">
                        {d.value.toLocaleString("en-US", { minimumFractionDigits: 2 })}
                        {" "}({d.pct.toFixed(1)}%)
                      </p>
                    </div>
                  );
                }}
              />
            </PieChart>
          </ResponsiveContainer>

          {/* Center label */}
          <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center">
            <p className="font-mono text-2xl font-bold text-text-primary">
              {formatUsd(total)}
            </p>
            <p className="text-xs text-text-tertiary">TVL</p>
          </div>
        </div>
      </div>

      {/* Legend with token details */}
      <div className="space-y-2.5">
        {TOKENS.slice(0, pool.nAssets).map((token, i) => {
          const amount = reserves[i];
          const pct = total > 0 ? (amount / total) * 100 : 100 / pool.nAssets;
          const deviation = Math.abs(pct - idealPct);
          const isDepegged = deviation > 5;

          return (
            <div key={token.symbol} className="flex items-center justify-between">
              <div className="flex items-center gap-2.5">
                <span
                  className="h-3 w-3 rounded-full"
                  style={{ backgroundColor: TOKEN_COLORS[i] }}
                />
                <span className="text-sm font-medium text-text-primary">
                  {token.symbol}
                </span>
              </div>
              <div className="flex items-center gap-3">
                <span className="font-mono text-sm text-text-secondary">
                  {amount.toLocaleString("en-US", {
                    minimumFractionDigits: 2,
                    maximumFractionDigits: 2,
                  })}
                </span>
                <span
                  className={`w-14 text-right font-mono text-xs ${
                    isDepegged ? "font-semibold text-warning" : "text-text-tertiary"
                  }`}
                >
                  {pct.toFixed(1)}%
                </span>
              </div>
            </div>
          );
        })}
      </div>

      {/* Depeg legend */}
      <div className="mt-3 flex items-center gap-2 text-[10px] text-text-tertiary">
        <span className="h-1.5 w-1.5 rounded-full bg-warning" />
        <span>Depeg warning: &gt;5% deviation from equal weight</span>
      </div>
    </Card>
  );
}
