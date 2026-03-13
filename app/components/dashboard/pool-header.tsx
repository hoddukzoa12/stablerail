"use client";

import { Badge } from "../ui/badge";
import { POOL_PDA } from "../../lib/devnet-config";
import { TOKENS } from "../../lib/tokens";
import { truncateAddress, explorerUrl } from "../../lib/format-utils";
import type { PoolState } from "../../lib/stablerail-math";

interface PoolHeaderProps {
  pool: PoolState;
}

export function PoolHeader({ pool }: PoolHeaderProps) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      {/* Token icons cluster */}
      <div className="flex -space-x-2">
        {TOKENS.slice(0, pool.nAssets).map((token) => (
          <div
            key={token.symbol}
            className="flex h-9 w-9 items-center justify-center rounded-full border-2 border-surface-base"
            style={{ backgroundColor: token.colorHex }}
          >
            <span className="text-xs font-bold text-white">
              {token.symbol.charAt(0)}
            </span>
          </div>
        ))}
      </div>

      {/* Pool name */}
      <h1 className="text-xl font-bold text-text-primary">
        {TOKENS.slice(0, pool.nAssets)
          .map((t) => t.symbol)
          .join(" / ")}
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
        href={explorerUrl("address", POOL_PDA)}
        target="_blank"
        rel="noopener noreferrer"
        className="font-mono text-xs text-text-tertiary transition-colors hover:text-text-secondary"
        title="View on Solana Explorer"
      >
        {truncateAddress(POOL_PDA, 6, 4)} ↗
      </a>
    </div>
  );
}
