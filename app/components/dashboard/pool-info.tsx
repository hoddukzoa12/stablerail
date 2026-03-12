"use client";

import { useState } from "react";
import { Card } from "../ui/card";
import { Badge } from "../ui/badge";
import { POOL_PDA } from "../../lib/devnet-config";
import type { PoolState } from "../../lib/stablerail-math";

interface PoolInfoProps {
  pool: PoolState;
}

function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export function PoolInfo({ pool }: PoolInfoProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(POOL_PDA);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API may fail in some contexts
    }
  };

  const Q64 = 1n << 64n;
  const radiusDisplay = Number(pool.radius.raw / Q64);

  return (
    <Card variant="glass" className="p-5">
      <h3 className="mb-4 text-sm font-semibold text-text-primary">
        Pool Info
      </h3>

      <div className="grid grid-cols-2 gap-x-6 gap-y-3 sm:grid-cols-4">
        {/* Pool Address */}
        <div>
          <p className="text-xs text-text-tertiary">Pool Address</p>
          <button
            onClick={handleCopy}
            className="mt-0.5 font-mono text-sm text-text-secondary transition-colors hover:text-text-primary cursor-pointer"
            title="Click to copy"
          >
            {copied ? "Copied!" : truncateAddress(POOL_PDA)}
          </button>
        </div>

        {/* Sphere Radius */}
        <div>
          <p className="text-xs text-text-tertiary">Sphere Radius</p>
          <p className="mt-0.5 font-mono text-sm text-text-primary">
            {radiusDisplay.toLocaleString("en-US", {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            })}
          </p>
        </div>

        {/* Status */}
        <div>
          <p className="text-xs text-text-tertiary">Status</p>
          <div className="mt-0.5">
            <Badge variant={pool.isActive ? "success" : "error"}>
              {pool.isActive ? "Active" : "Paused"}
            </Badge>
          </div>
        </div>

        {/* Position Count */}
        <div>
          <p className="text-xs text-text-tertiary">LP Positions</p>
          <p className="mt-0.5 font-mono text-sm text-text-primary">
            {pool.positionCount}
          </p>
        </div>
      </div>
    </Card>
  );
}
