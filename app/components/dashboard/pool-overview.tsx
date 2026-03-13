"use client";

import { Card } from "../ui/card";
import { q6464ToNumber, formatUsd } from "../../lib/format-utils";
import type { PoolState } from "../../lib/stablerail-math";

interface PoolOverviewProps {
  pool: PoolState;
}

export function PoolOverview({ pool }: PoolOverviewProps) {
  const tvl = pool.reserves.reduce(
    (sum, reserve) => sum + q6464ToNumber(reserve.raw),
    0,
  );
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
