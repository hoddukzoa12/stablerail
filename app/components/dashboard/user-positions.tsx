"use client";

import { useState } from "react";
import { Card } from "../ui/card";
import { Button } from "../ui/button";
import { useRemoveLiquidity } from "../../hooks/useRemoveLiquidity";
import type { UserPosition } from "../../hooks/useUserPositions";
import type { Transaction } from "../../hooks/useTransactionHistory";

type ActiveTab = "positions" | "transactions";

interface UserPositionsProps {
  positions: UserPosition[];
  isLoading: boolean;
  onRemoveSuccess: () => void;
  /** Transaction history data */
  transactions: Transaction[];
  /** Whether transaction history is loading */
  isLoadingTransactions: boolean;
}

function formatDate(unixSeconds: number): string {
  if (unixSeconds <= 0) return "Unknown";
  return new Date(unixSeconds * 1000).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function truncateAddress(address: string): string {
  if (address.length <= 10) return address;
  return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

/** Icon + color per transaction type */
const TX_TYPE_CONFIG: Record<
  Transaction["type"],
  { icon: string; color: string }
> = {
  Swap: { icon: "\u{1F504}", color: "text-accent-blue" },
  "Add Liquidity": { icon: "\u{2795}", color: "text-success" },
  "Remove Liquidity": { icon: "\u{2796}", color: "text-warning" },
  Settlement: { icon: "\u{1F3DB}", color: "text-accent-purple" },
  Unknown: { icon: "\u{2753}", color: "text-text-tertiary" },
};

export function UserPositions({
  positions,
  isLoading,
  onRemoveSuccess,
  transactions,
  isLoadingTransactions,
}: UserPositionsProps) {
  const { execute, isSending, error } = useRemoveLiquidity();
  const [removingAddress, setRemovingAddress] = useState<string | null>(null);
  const [txResult, setTxResult] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<ActiveTab>("positions");

  // ── Aggregate state ──
  const [expanded, setExpanded] = useState(false);
  const [aggregatePercent, setAggregatePercent] = useState(100);
  const [aggregateRemoving, setAggregateRemoving] = useState(false);
  const [removeProgress, setRemoveProgress] = useState<string | null>(null);

  // ── Per-position state (used in expanded view) ──
  const [removePercent, setRemovePercent] = useState<Record<string, number>>(
    {},
  );

  // Filter out fully withdrawn (liquidity = 0) positions
  const activePositions = positions.filter((p) => p.liquidityRaw > 0n);

  // Aggregate totals
  const totalLiquidity = activePositions.reduce(
    (sum, p) => sum + p.liquidityDisplay,
    0,
  );

  const getPercent = (address: string) => removePercent[address] ?? 100;

  // ── Aggregate remove: apply same % to all positions sequentially ──
  const handleAggregateRemove = async () => {
    setAggregateRemoving(true);
    setTxResult(null);
    const total = activePositions.length;
    let lastSig: string | null = null;

    for (let i = 0; i < total; i++) {
      setRemoveProgress(`Removing ${i + 1}/${total}...`);
      const pos = activePositions[i];
      const partialRaw =
        aggregatePercent === 100
          ? pos.liquidityRaw
          : (pos.liquidityRaw * BigInt(aggregatePercent)) / 100n;

      try {
        const sig = await execute({
          positionAddress: pos.address,
          liquidityRaw: partialRaw,
        });
        lastSig = sig;
      } catch {
        break; // stop on first failure — error tracked in hook
      }
    }

    if (lastSig) setTxResult(lastSig);
    setAggregateRemoving(false);
    setRemoveProgress(null);
    onRemoveSuccess();
  };

  // ── Individual position remove (expanded view) ──
  const handleRemove = async (position: UserPosition) => {
    setRemovingAddress(position.address);
    setTxResult(null);

    const pct = getPercent(position.address);
    const partialRaw =
      pct === 100
        ? position.liquidityRaw
        : (position.liquidityRaw * BigInt(pct)) / 100n;

    try {
      const sig = await execute({
        positionAddress: position.address,
        liquidityRaw: partialRaw,
      });
      setTxResult(sig);
      onRemoveSuccess();
    } catch {
      // error tracked in hook
    } finally {
      setRemovingAddress(null);
    }
  };

  return (
    <Card variant="glass" className="flex max-h-[420px] flex-col p-5">
      {/* Tab button group */}
      <div className="mb-4 flex items-center gap-1 rounded-lg bg-surface-2 p-1">
        <button
          type="button"
          className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
            activeTab === "positions"
              ? "bg-surface-3 text-text-primary shadow-sm"
              : "text-text-tertiary hover:text-text-secondary"
          }`}
          onClick={() => setActiveTab("positions")}
        >
          Positions
          {activePositions.length > 0 && (
            <span className="ml-1.5 rounded-full bg-accent-purple/20 px-1.5 py-0.5 text-[10px] font-semibold text-accent-purple">
              {activePositions.length}
            </span>
          )}
        </button>
        <button
          type="button"
          className={`flex-1 rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
            activeTab === "transactions"
              ? "bg-surface-3 text-text-primary shadow-sm"
              : "text-text-tertiary hover:text-text-secondary"
          }`}
          onClick={() => setActiveTab("transactions")}
        >
          Transactions
          {transactions.length > 0 && (
            <span className="ml-1.5 rounded-full bg-accent-blue/20 px-1.5 py-0.5 text-[10px] font-semibold text-accent-blue">
              {transactions.length}
            </span>
          )}
        </button>
      </div>

      {/* ── Positions tab ── */}
      {activeTab === "positions" && (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {isLoading && positions.length === 0 && (
            <p className="py-6 text-center text-sm text-text-tertiary">
              Loading positions...
            </p>
          )}

          {!isLoading && activePositions.length === 0 && (
            <p className="py-6 text-center text-sm text-text-tertiary">
              No active positions. Add liquidity to get started.
            </p>
          )}

          {/* ── Aggregate card ── */}
          {activePositions.length > 0 && (
            <div className="space-y-3">
              <div className="rounded-lg bg-surface-2 p-4">
                {/* Total liquidity header */}
                <div className="flex items-start justify-between">
                  <div>
                    <p className="text-xs text-text-tertiary">
                      Total Liquidity
                    </p>
                    <p className="mt-0.5 font-mono text-lg font-semibold text-text-primary">
                      {totalLiquidity.toLocaleString("en-US", {
                        minimumFractionDigits: 2,
                        maximumFractionDigits: 4,
                      })}
                    </p>
                  </div>
                  <span className="rounded-full bg-accent-purple/10 px-2.5 py-1 text-xs font-medium text-accent-purple">
                    {activePositions.length}{" "}
                    {activePositions.length === 1 ? "position" : "positions"}
                  </span>
                </div>

                {/* Aggregate percentage selector + remove button */}
                <div className="mt-3 space-y-2">
                  {/* Preset buttons + custom input row */}
                  <div className="flex items-center gap-1.5">
                    {[25, 50, 75, 100].map((pct) => (
                      <button
                        key={pct}
                        type="button"
                        onClick={() => setAggregatePercent(pct)}
                        className={`flex-1 rounded-md py-1.5 text-[11px] font-semibold transition-colors ${
                          aggregatePercent === pct
                            ? "bg-brand-primary/20 text-brand-primary"
                            : "bg-surface-3 text-text-tertiary hover:text-text-secondary"
                        }`}
                      >
                        {pct}%
                      </button>
                    ))}
                    {/* Custom percentage input */}
                    <div className="relative flex w-16 shrink-0 items-center">
                      <input
                        type="number"
                        min={1}
                        max={100}
                        value={aggregatePercent}
                        onChange={(e) => {
                          const v = Math.max(1, Math.min(100, Number(e.target.value) || 1));
                          setAggregatePercent(v);
                        }}
                        className="w-full rounded-md bg-surface-3 py-1.5 pl-2 pr-5 font-mono text-[11px] font-semibold text-text-primary outline-none focus:ring-1 focus:ring-brand-primary/40"
                      />
                      <span className="pointer-events-none absolute right-1.5 text-[10px] text-text-tertiary">
                        %
                      </span>
                    </div>
                  </div>
                  {/* Remove button */}
                  <Button
                    variant="secondary"
                    size="sm"
                    className="w-full"
                    disabled={aggregateRemoving || isSending}
                    onClick={handleAggregateRemove}
                  >
                    {aggregateRemoving
                      ? removeProgress ?? "Removing..."
                      : `Remove ${aggregatePercent}%`}
                  </Button>
                </div>

                {/* Expand/collapse toggle */}
                <button
                  type="button"
                  onClick={() => setExpanded((prev) => !prev)}
                  className="mt-3 flex w-full items-center gap-1.5 text-xs text-text-tertiary transition-colors hover:text-text-secondary"
                >
                  <svg
                    className={`h-3 w-3 transition-transform ${expanded ? "rotate-90" : ""}`}
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2.5"
                  >
                    <path d="M9 5l7 7-7 7" />
                  </svg>
                  {expanded
                    ? "Hide individual positions"
                    : "View individual positions"}
                </button>
              </div>

              {/* ── Individual positions (expanded) ── */}
              {expanded && (
                <div className="space-y-2 pl-1">
                  {activePositions.map((pos, i) => (
                    <div
                      key={pos.address}
                      className="rounded-lg border border-border-default/50 bg-surface-2/60 p-3"
                    >
                      <div className="flex items-start justify-between">
                        <div>
                          <p className="text-xs font-medium text-text-secondary">
                            Position #{activePositions.length - i}
                          </p>
                          <a
                            href={`https://explorer.solana.com/address/${pos.address}?cluster=devnet`}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="mt-0.5 font-mono text-[11px] text-text-tertiary underline-offset-2 hover:text-accent-blue hover:underline"
                          >
                            {truncateAddress(pos.address)} ↗
                          </a>
                        </div>
                        <span className="text-[11px] text-text-tertiary">
                          {formatDate(pos.createdAt)}
                        </span>
                      </div>

                      <div className="mt-2">
                        <div className="flex items-center justify-between">
                          <p className="font-mono text-sm font-medium text-text-primary">
                            {pos.liquidityDisplay.toLocaleString("en-US", {
                              minimumFractionDigits: 2,
                              maximumFractionDigits: 4,
                            })}
                          </p>
                          <Button
                            variant="secondary"
                            size="sm"
                            disabled={
                              isSending && removingAddress === pos.address
                            }
                            onClick={() => handleRemove(pos)}
                          >
                            {isSending && removingAddress === pos.address
                              ? "Removing..."
                              : `Remove ${getPercent(pos.address)}%`}
                          </Button>
                        </div>

                        {/* Per-position percentage selector */}
                        <div className="mt-1.5 flex items-center gap-1">
                          {[25, 50, 75, 100].map((pct) => (
                            <button
                              key={pct}
                              type="button"
                              onClick={() =>
                                setRemovePercent((prev) => ({
                                  ...prev,
                                  [pos.address]: pct,
                                }))
                              }
                              className={`flex-1 rounded-md py-0.5 text-[10px] font-semibold transition-colors ${
                                getPercent(pos.address) === pct
                                  ? "bg-brand-primary/20 text-brand-primary"
                                  : "bg-surface-3 text-text-tertiary hover:text-text-secondary"
                              }`}
                            >
                              {pct}%
                            </button>
                          ))}
                          <div className="relative flex w-14 shrink-0 items-center">
                            <input
                              type="number"
                              min={1}
                              max={100}
                              value={getPercent(pos.address)}
                              onChange={(e) => {
                                const v = Math.max(1, Math.min(100, Number(e.target.value) || 1));
                                setRemovePercent((prev) => ({
                                  ...prev,
                                  [pos.address]: v,
                                }));
                              }}
                              className="w-full rounded-md bg-surface-3 py-0.5 pl-1.5 pr-4 font-mono text-[10px] font-semibold text-text-primary outline-none focus:ring-1 focus:ring-brand-primary/40"
                            />
                            <span className="pointer-events-none absolute right-1 text-[9px] text-text-tertiary">
                              %
                            </span>
                          </div>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="mt-2 rounded-lg bg-error/10 px-3 py-2 text-center text-xs text-error">
              {error.message}
            </div>
          )}

          {/* Success */}
          {txResult && (
            <div className="mt-2 rounded-lg bg-success/10 px-3 py-2 text-center text-xs text-success">
              Liquidity removed!{" "}
              <a
                href={`https://explorer.solana.com/tx/${txResult}?cluster=devnet`}
                target="_blank"
                rel="noopener noreferrer"
                className="underline underline-offset-2"
              >
                View on Explorer
              </a>
            </div>
          )}
        </div>
      )}

      {/* ── Transactions tab ── */}
      {activeTab === "transactions" && (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {isLoadingTransactions && transactions.length === 0 && (
            <p className="py-6 text-center text-sm text-text-tertiary">
              Loading transactions...
            </p>
          )}

          {!isLoadingTransactions && transactions.length === 0 && (
            <p className="py-6 text-center text-sm text-text-tertiary">
              No transactions yet.
            </p>
          )}

          <div className="space-y-2">
            {transactions.map((tx) => {
              const config = TX_TYPE_CONFIG[tx.type];
              return (
                <div
                  key={tx.signature}
                  className="rounded-lg bg-surface-2 p-3"
                >
                  <div className="flex items-start justify-between">
                    <div className="flex items-center gap-2">
                      <span className="text-sm" role="img" aria-label={tx.type}>
                        {config.icon}
                      </span>
                      <span className={`text-sm font-medium ${config.color}`}>
                        {tx.type}
                      </span>
                    </div>
                    <span className="text-xs text-text-tertiary">
                      {formatDate(tx.timestamp)}
                    </span>
                  </div>

                  <div className="mt-1.5 flex items-center justify-between">
                    <a
                      href={`https://explorer.solana.com/tx/${tx.signature}?cluster=devnet`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="font-mono text-xs text-text-tertiary underline-offset-2 hover:text-accent-blue hover:underline"
                    >
                      {truncateAddress(tx.signature)}
                    </a>
                    <span
                      className={`text-xs font-medium ${
                        tx.status === "success"
                          ? "text-success"
                          : "text-error"
                      }`}
                    >
                      {tx.status === "success" ? "Success" : "Failed"}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}
    </Card>
  );
}
