"use client";

import { useState } from "react";
import Link from "next/link";
import { useWalletConnection } from "@solana/react-hooks";
import { usePoolState } from "../hooks/usePoolState";
import { useTokenBalances } from "../hooks/useTokenBalances";
import { useUserPositions } from "../hooks/useUserPositions";
import { useTransactionHistory } from "../hooks/useTransactionHistory";
import { PoolHeader } from "../components/dashboard/pool-header";
import { PoolOverview } from "../components/dashboard/pool-overview";
import { ReserveChart } from "../components/dashboard/reserve-chart";
import { AddLiquidityForm } from "../components/dashboard/add-liquidity-form";
import { UserPositions } from "../components/dashboard/user-positions";
import { TokensCard } from "../components/dashboard/tokens-card";
import { Modal } from "../components/ui/modal";
import { Button } from "../components/ui/button";

export default function DashboardPage() {
  const { status } = useWalletConnection();
  const { pool, isLoading: poolLoading } = usePoolState();
  const { balances, refresh: refreshBalances } = useTokenBalances();
  const {
    positions,
    isLoading: positionsLoading,
    refresh: refreshPositions,
  } = useUserPositions();
  const {
    transactions,
    isLoading: txLoading,
  } = useTransactionHistory();

  const [liquidityModalOpen, setLiquidityModalOpen] = useState(false);

  const isConnected = status === "connected";

  const handleLiquiditySuccess = () => {
    refreshBalances();
    refreshPositions();
  };

  const openLiquidityModal = () => setLiquidityModalOpen(true);
  const closeLiquidityModal = () => setLiquidityModalOpen(false);

  // Loading state
  if (poolLoading) {
    return (
      <div className="mx-auto flex w-full max-w-6xl items-center justify-center px-4 py-20">
        <div className="text-sm text-text-tertiary">Loading pool data...</div>
      </div>
    );
  }

  // Pool not found
  if (!pool) {
    return (
      <div className="mx-auto flex w-full max-w-6xl items-center justify-center px-4 py-20">
        <div className="text-sm text-text-tertiary">
          Pool not found. Make sure devnet is accessible.
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-6xl px-4 py-6">
      {/* Pool Header — Uniswap style */}
      <PoolHeader pool={pool} />

      {/* 2-Column Layout: Left (chart) + Right (actions + stats) */}
      <div className="mt-6 grid gap-5 md:grid-cols-[1fr_340px]">
        {/* Left column — Reserve distribution (main content area) */}
        <ReserveChart pool={pool} />

        {/* Right column — Actions + Stats */}
        <div className="space-y-4">
          {/* Action buttons — Uniswap style */}
          <div className="flex gap-2">
            <Link href="/" className="flex-1">
              <Button variant="secondary" size="md" className="w-full gap-1.5">
                <svg className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                  <path d="M7 16V4m0 0L3 8m4-4l4 4M17 8v12m0 0l4-4m-4 4l-4-4" />
                </svg>
                Swap
              </Button>
            </Link>
            <Button
              variant="primary"
              size="md"
              className="flex-1 gap-1.5"
              onClick={openLiquidityModal}
            >
              <svg className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <path d="M12 5v14m-7-7h14" />
              </svg>
              Add liquidity
            </Button>
          </div>

          {/* Stats card */}
          <PoolOverview pool={pool} />
        </div>
      </div>

      {/* Bottom section: Positions (left) + Tokens (right) */}
      <div className="mt-6">
        {isConnected ? (
          <div className="grid gap-5 md:grid-cols-[1fr_340px]">
            <UserPositions
              positions={positions}
              isLoading={positionsLoading}
              onRemoveSuccess={handleLiquiditySuccess}
              transactions={transactions}
              isLoadingTransactions={txLoading}
            />
            <TokensCard balances={balances} />
          </div>
        ) : (
          <div className="rounded-xl border border-border-default bg-surface-1 p-8 text-center">
            <p className="text-sm text-text-secondary">
              Connect your wallet to add liquidity and manage positions.
            </p>
          </div>
        )}
      </div>

      {/* Add Liquidity Modal */}
      <Modal
        open={liquidityModalOpen}
        onClose={closeLiquidityModal}
        title="Add Liquidity"
      >
        <AddLiquidityForm
          pool={pool}
          balances={balances}
          onSuccess={() => {
            handleLiquiditySuccess();
            closeLiquidityModal();
          }}
        />
      </Modal>
    </div>
  );
}
