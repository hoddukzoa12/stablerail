"use client";

import { Badge } from "../ui/badge";
import { POOL_PDA } from "../../lib/devnet-config";
import type { PoolState } from "../../lib/stablerail-math";

interface PoolHeaderProps {
  pool: PoolState;
}

function truncateAddress(address: string): string {
  if (address.length <= 12) return address;
  return `${address.slice(0, 6)}...${address.slice(-4)}`;
}

export function PoolHeader({ pool }: PoolHeaderProps) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      {/* Token icons cluster */}
      <div className="flex -space-x-2">
        <div className="flex h-9 w-9 items-center justify-center rounded-full border-2 border-surface-base bg-[#2775CA]">
          <span className="text-xs font-bold text-white">U</span>
        </div>
        <div className="flex h-9 w-9 items-center justify-center rounded-full border-2 border-surface-base bg-[#26A17B]">
          <span className="text-xs font-bold text-white">T</span>
        </div>
        <div className="flex h-9 w-9 items-center justify-center rounded-full border-2 border-surface-base bg-[#0033A0]">
          <span className="text-xs font-bold text-white">P</span>
        </div>
      </div>

      {/* Pool name */}
      <h1 className="text-xl font-bold text-text-primary">
        USDC / USDT / PYUSD
      </h1>

      {/* Badges */}
      <div className="flex items-center gap-1.5">
        <span className="rounded-md bg-surface-3 px-2 py-0.5 text-xs font-medium text-text-secondary">
          v1
        </span>
        <span className="rounded-md bg-surface-3 px-2 py-0.5 text-xs font-medium text-text-secondary">
          {(pool.feeRateBps / 100).toFixed(2)}%
        </span>
        <Badge variant={pool.isActive ? "success" : "error"}>
          {pool.isActive ? "Active" : "Paused"}
        </Badge>
      </div>

      {/* Address → Explorer link */}
      <a
        href={`https://explorer.solana.com/address/${POOL_PDA}?cluster=devnet`}
        target="_blank"
        rel="noopener noreferrer"
        className="font-mono text-xs text-text-tertiary transition-colors hover:text-text-secondary"
        title="View on Solana Explorer"
      >
        {truncateAddress(POOL_PDA)} ↗
      </a>
    </div>
  );
}
