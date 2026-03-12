"use client";

import { Card } from "../ui/card";
import type { PoolState } from "../../lib/stablerail-math";

interface PoolOverviewProps {
  pool: PoolState;
}

/** Convert Q64.64 value to human-readable number.
 *  On-chain values are already decimal-normalized via from_token_amount(),
 *  so raw / Q64 gives the whole-token amount directly (e.g., 102 USDC). */
function q6464ToNumber(raw: bigint): number {
  const Q64 = 1n << 64n;
  const intPart = Number(raw / Q64);
  const fracPart = Number(raw % Q64) / Number(Q64);
  return intPart + fracPart;
}

function formatUsd(value: number): string {
  if (value >= 1_000_000) {
    return `$${(value / 1_000_000).toFixed(1)}M`;
  }
  if (value >= 1_000) {
    return `$${(value / 1_000).toFixed(1)}K`;
  }
  return `$${value.toFixed(2)}`;
}

export function PoolOverview({ pool }: PoolOverviewProps) {
  // TVL = sum of all reserves (already in whole-token units)
  const tvl = pool.reserves.reduce(
    (sum, reserve) => sum + q6464ToNumber(reserve.raw),
    0,
  );

  // Volume and fees are also decimal-normalized FixedPoint
  const volumeDisplay = q6464ToNumber(pool.totalVolume.raw);
  const feesDisplay = q6464ToNumber(pool.totalFees.raw);

  return (
    <Card variant="glass" className="p-5">
      <h3 className="mb-4 text-sm font-semibold text-text-primary">Stats</h3>

      <div className="space-y-4">
        <div>
          <p className="text-xs text-text-tertiary">TVL</p>
          <p className="font-mono text-xl font-bold text-text-primary">
            {formatUsd(tvl)}
          </p>
        </div>

        <div>
          <p className="text-xs text-text-tertiary">Total volume</p>
          <p className="font-mono text-xl font-bold text-text-primary">
            {formatUsd(volumeDisplay)}
          </p>
          <p className="text-xs text-text-tertiary">all-time</p>
        </div>

        <div>
          <p className="text-xs text-text-tertiary">Total fees</p>
          <p className="font-mono text-xl font-bold text-text-primary">
            {formatUsd(feesDisplay)}
          </p>
          <p className="text-xs text-text-tertiary">all-time</p>
        </div>

        <div>
          <p className="text-xs text-text-tertiary">LP Positions</p>
          <p className="font-mono text-lg font-semibold text-text-primary">
            {pool.positionCount}
          </p>
        </div>

        <div>
          <p className="text-xs text-text-tertiary">Sphere Radius</p>
          <p className="font-mono text-sm text-text-secondary">
            {q6464ToNumber(pool.radius.raw).toLocaleString("en-US", {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            })}
          </p>
        </div>
      </div>
    </Card>
  );
}
