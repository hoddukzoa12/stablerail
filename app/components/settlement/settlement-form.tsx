"use client";

import { useState, useMemo } from "react";
import { Card } from "../ui/card";
import { Button } from "../ui/button";
import { TxNotification } from "../ui/tx-notification";
import { useExecuteSettlement } from "../../hooks/useExecuteSettlement";
import { usePoolState } from "../../hooks/usePoolState";
import { TOKENS } from "../../lib/tokens";
import { formatAmount } from "../../lib/format-utils";
import type { PolicyStateData } from "../../lib/settlement-deserializer";
import { PolicyCompliancePreview } from "./policy-compliance-preview";
import { ArrowDownUp } from "lucide-react";

interface SettlementFormProps {
  policy: PolicyStateData;
  tokenBalances: Record<string, bigint>;
  onSuccess: () => void;
}

const TOKEN_SELECT_CLASSES =
  "rounded-lg bg-surface-3 px-3 py-2 text-sm font-medium text-text-primary outline-none";

export function SettlementForm({ policy, tokenBalances, onSuccess }: SettlementFormProps) {
  const { execute, isSending, signature, error } = useExecuteSettlement();
  const { pool } = usePoolState();

  const [tokenInIndex, setTokenInIndex] = useState(0);
  const [tokenOutIndex, setTokenOutIndex] = useState(1);
  const [amount, setAmount] = useState("");
  const [slippage, setSlippage] = useState("0.5");

  const tokenIn = TOKENS[tokenInIndex];
  const tokenOut = TOKENS[tokenOutIndex];

  const amountNum = parseFloat(amount) || 0;
  const slippageNum = parseFloat(slippage) || 0.5;

  const estimatedOut = useMemo(() => {
    if (!pool || amountNum <= 0) return 0;
    const feeMultiplier = 1 - pool.feeRateBps / 10000;
    return amountNum * feeMultiplier;
  }, [pool, amountNum]);

  const minAmountOut = useMemo(() => {
    return estimatedOut * (1 - slippageNum / 100);
  }, [estimatedOut, slippageNum]);

  const balanceIn = tokenBalances[tokenIn.symbol] ?? 0n;
  const balanceDisplay = Number(balanceIn) / 10 ** tokenIn.decimals;

  function handleSwapDirection(): void {
    setTokenInIndex(tokenOutIndex);
    setTokenOutIndex(tokenInIndex);
    setAmount("");
  }

  function handleMax(): void {
    setAmount(balanceDisplay.toFixed(2));
  }

  async function handleSubmit(e: React.FormEvent): Promise<void> {
    e.preventDefault();
    if (amountNum <= 0) return;

    const amountU64 = BigInt(Math.floor(amountNum * 10 ** tokenIn.decimals));
    const minOutU64 = BigInt(Math.floor(minAmountOut * 10 ** tokenOut.decimals));

    try {
      await execute({
        tokenInIndex: tokenIn.index,
        tokenOutIndex: tokenOut.index,
        amount: amountU64,
        minAmountOut: minOutU64,
        vaultIn: tokenIn.vault,
        vaultOut: tokenOut.vault,
        mintIn: tokenIn.mint,
        mintOut: tokenOut.mint,
      });
      setAmount("");
      onSuccess();
    } catch {
      // Error state managed by useExecuteSettlement hook
    }
  }

  return (
    <div className="space-y-4">
      <Card variant="glass">
        <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
          Execute Settlement
        </h3>

        <form onSubmit={handleSubmit} className="mt-4 space-y-4">
          <div className="rounded-xl bg-surface-2 p-4">
            <div className="mb-2 flex items-center justify-between">
              <span className="text-xs text-text-tertiary">You send</span>
              <button
                type="button"
                onClick={handleMax}
                className="text-xs text-brand-primary hover:underline"
              >
                Balance: {formatAmount(balanceDisplay)}
              </button>
            </div>
            <div className="flex items-center gap-3">
              <select
                value={tokenInIndex}
                onChange={(e) => {
                  const idx = Number(e.target.value);
                  if (idx === tokenOutIndex) setTokenOutIndex(tokenInIndex);
                  setTokenInIndex(idx);
                }}
                className={TOKEN_SELECT_CLASSES}
              >
                {TOKENS.map((t, i) => (
                  <option key={t.symbol} value={i}>{t.symbol}</option>
                ))}
              </select>
              <input
                type="text"
                inputMode="decimal"
                value={amount}
                onChange={(e) => setAmount(e.target.value)}
                placeholder="0.00"
                className="w-full bg-transparent text-right font-mono text-xl text-text-primary outline-none"
              />
            </div>
          </div>

          <div className="flex justify-center">
            <button
              type="button"
              onClick={handleSwapDirection}
              className="rounded-full bg-surface-2 p-2 transition-colors hover:bg-surface-3"
            >
              <ArrowDownUp className="h-4 w-4 text-text-tertiary" />
            </button>
          </div>

          <div className="rounded-xl bg-surface-2 p-4">
            <div className="mb-2">
              <span className="text-xs text-text-tertiary">You receive</span>
            </div>
            <div className="flex items-center gap-3">
              <select
                value={tokenOutIndex}
                onChange={(e) => {
                  const idx = Number(e.target.value);
                  if (idx === tokenInIndex) setTokenInIndex(tokenOutIndex);
                  setTokenOutIndex(idx);
                }}
                className={TOKEN_SELECT_CLASSES}
              >
                {TOKENS.map((t, i) => (
                  <option key={t.symbol} value={i}>{t.symbol}</option>
                ))}
              </select>
              <div className="w-full text-right font-mono text-xl text-text-secondary">
                {formatAmount(estimatedOut)}
              </div>
            </div>
          </div>

          <div className="flex items-center justify-between rounded-lg bg-surface-2 px-4 py-2">
            <span className="text-xs text-text-tertiary">
              Slippage Tolerance
            </span>
            <div className="flex items-center gap-1">
              <input
                type="text"
                inputMode="decimal"
                value={slippage}
                onChange={(e) => setSlippage(e.target.value)}
                className="w-12 bg-transparent text-right font-mono text-sm text-text-primary outline-none"
              />
              <span className="text-xs text-text-tertiary">%</span>
            </div>
          </div>

          {amountNum > 0 && (
            <div className="flex items-center justify-between px-1">
              <span className="text-xs text-text-tertiary">
                Min. received
              </span>
              <span className="font-mono text-xs text-text-secondary">
                {formatAmount(minAmountOut)} {tokenOut.symbol}
              </span>
            </div>
          )}

          <Button
            type="submit"
            variant="gradient"
            size="lg"
            className="w-full"
            disabled={isSending || amountNum <= 0}
          >
            {isSending ? "Executing Settlement..." : "Execute Settlement"}
          </Button>
        </form>

        <TxNotification
          error={error}
          txSignature={signature}
          successLabel="Settlement executed!"
        />
      </Card>

      {amountNum > 0 && (
        <PolicyCompliancePreview policy={policy} amount={amountNum} />
      )}
    </div>
  );
}
