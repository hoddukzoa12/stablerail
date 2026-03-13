"use client";

import { useState, useCallback, useMemo } from "react";
import { Button } from "../ui/button";
import { TxNotification } from "../ui/tx-notification";
import { TOKENS } from "../../lib/tokens";
import { q6464ToNumber, formatBalance } from "../../lib/format-utils";
import { useAddLiquidity } from "../../hooks/useAddLiquidity";
import type { PoolState } from "../../lib/stablerail-math";

interface AddLiquidityFormProps {
  pool: PoolState;
  balances: Record<string, bigint>;
  onSuccess: () => void;
}

/** Get token balance as a number (whole-token units). */
function balanceToNumber(
  balances: Record<string, bigint>,
  symbol: string,
  decimals: number,
): number {
  return Number(balances[symbol] ?? 0n) / 10 ** decimals;
}

/** Determine the submit button label based on form state. */
function getSubmitLabel(
  isSending: boolean,
  hasAnyInput: boolean,
  allPositive: boolean,
  exceedsBalance: boolean,
): string {
  if (isSending) return "Adding Liquidity...";
  if (!hasAnyInput) return "Enter amounts";
  if (!allPositive) return "All tokens required";
  if (exceedsBalance) return "Insufficient balance";
  return "Add Liquidity";
}

export function AddLiquidityForm({
  pool,
  balances,
  onSuccess,
}: AddLiquidityFormProps) {
  const tokens = TOKENS.slice(0, pool.nAssets);
  const [amounts, setAmounts] = useState<string[]>(tokens.map(() => ""));
  const [txResult, setTxResult] = useState<string | null>(null);
  const { execute, isSending, error } = useAddLiquidity();

  const reserves = useMemo(
    () => pool.reserves.map((r) => q6464ToNumber(r.raw)),
    [pool.reserves],
  );

  const updateAmount = (index: number, value: string) => {
    if (!/^[0-9]*\.?[0-9]*$/.test(value)) return;
    setAmounts((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
  };

  const handleMax = (index: number) => {
    const token = tokens[index];
    const raw = balanceToNumber(balances, token.symbol, token.decimals);
    if (raw <= 0) return;
    updateAmount(index, (Math.floor(raw * 100) / 100).toFixed(2));
  };

  const handleProportionalFill = () => {
    const anchorIdx = amounts.findIndex((a) => parseFloat(a || "0") > 0);
    if (anchorIdx === -1) return;

    const anchorAmount = parseFloat(amounts[anchorIdx]);
    if (anchorAmount <= 0 || reserves[anchorIdx] === 0) return;

    const ratio = anchorAmount / reserves[anchorIdx];

    const proportional = tokens.map((_, i) =>
      i === anchorIdx ? anchorAmount : reserves[i] * ratio,
    );

    const balanceRatios = proportional.map((p, i) => {
      const bal = balanceToNumber(balances, tokens[i].symbol, tokens[i].decimals);
      return p > 0 ? bal / p : Infinity;
    });
    const scale = Math.min(1, Math.min(...balanceRatios));

    setAmounts(
      proportional.map((p) => (Math.floor(p * scale * 100) / 100).toFixed(2)),
    );
  };

  const handleSubmit = useCallback(async () => {
    setTxResult(null);

    const baseAmounts = tokens.map((token, i) => {
      const val = parseFloat(amounts[i] || "0");
      return BigInt(Math.floor(val * 10 ** token.decimals));
    });

    if (baseAmounts.some((a) => a === 0n)) return;

    try {
      const sig = await execute({ amounts: baseAmounts }, pool);
      setTxResult(sig);
      setAmounts(tokens.map(() => ""));
      onSuccess();
    } catch {
      // error is tracked in the hook
    }
  }, [amounts, tokens, pool, execute, onSuccess]);

  // Validation
  const parsedAmounts = amounts.map((a) => parseFloat(a || "0"));
  const allPositive = parsedAmounts.every((a) => a > 0);
  const hasAnyInput = parsedAmounts.some((a) => a > 0);
  const hasZero = hasAnyInput && !allPositive;

  const exceedsBalance = tokens.some((token, i) => {
    return parsedAmounts[i] > balanceToNumber(balances, token.symbol, token.decimals);
  });

  return (
    <div>
      {/* Info banner */}
      <div className="mb-3 rounded-lg bg-accent-blue/10 px-3 py-2 text-[11px] text-accent-blue">
        Asymmetric deposits OK — all tokens need at least a minimal amount.
        The sphere invariant auto-adjusts.
      </div>

      <div className="space-y-3">
        {tokens.map((token, i) => {
          const bal = balanceToNumber(balances, token.symbol, token.decimals);
          const isOver = parsedAmounts[i] > bal;
          const isEmpty = hasAnyInput && parsedAmounts[i] === 0;

          return (
            <div
              key={token.symbol}
              className={`rounded-lg p-3 transition-colors ${
                isOver
                  ? "bg-error/10 ring-1 ring-error/30"
                  : isEmpty
                    ? "bg-warning/10 ring-1 ring-warning/30"
                    : "bg-surface-2"
              }`}
            >
              <div className="mb-1.5 flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span
                    className="h-2 w-2 rounded-full"
                    style={{ backgroundColor: token.colorHex }}
                  />
                  <span className="text-xs font-medium text-text-primary">
                    {token.symbol}
                  </span>
                </div>
                <span className="text-xs text-text-tertiary">
                  Balance:{" "}
                  {formatBalance(
                    balances[token.symbol] ?? 0n,
                    token.decimals,
                    "0.00",
                  )}
                </span>
              </div>

              <div className="flex items-center gap-2">
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder="0.00"
                  value={amounts[i]}
                  onChange={(e) => updateAmount(i, e.target.value)}
                  className="min-w-0 flex-1 bg-transparent font-mono text-lg font-semibold text-text-primary outline-none placeholder:text-text-tertiary/50"
                />
                {(balances[token.symbol] ?? 0n) > 0n && (
                  <button
                    type="button"
                    onClick={() => handleMax(i)}
                    className="cursor-pointer rounded-md bg-brand-primary/10 px-2 py-0.5 text-[10px] font-semibold uppercase text-brand-primary transition-colors hover:bg-brand-primary/20"
                  >
                    MAX
                  </button>
                )}
              </div>

              {isOver && (
                <p className="mt-1 text-[10px] text-error">Exceeds balance</p>
              )}
              {isEmpty && (
                <p className="mt-1 text-[10px] text-warning">
                  Required (min any amount &gt; 0)
                </p>
              )}
            </div>
          );
        })}
      </div>

      {/* Quick-fill buttons */}
      <div className="mt-3 flex gap-2">
        <button
          type="button"
          onClick={handleProportionalFill}
          disabled={!hasAnyInput}
          className="flex-1 cursor-pointer rounded-lg bg-surface-2 px-3 py-2 text-xs font-medium text-text-secondary transition-colors hover:bg-surface-3 hover:text-text-primary disabled:cursor-not-allowed disabled:opacity-40"
        >
          Proportional Fill
        </button>
      </div>

      {hasZero && (
        <div className="mt-2 rounded-lg bg-warning/10 px-3 py-2 text-center text-[11px] text-warning">
          Each token needs at least a minimal deposit (can be asymmetric).
        </div>
      )}

      <Button
        variant="gradient"
        size="lg"
        className="mt-4 w-full"
        disabled={!allPositive || exceedsBalance || isSending}
        onClick={handleSubmit}
      >
        {getSubmitLabel(isSending, hasAnyInput, allPositive, exceedsBalance)}
      </Button>

      <TxNotification
        error={error}
        txSignature={txResult}
        successLabel="Liquidity added!"
      />
    </div>
  );
}
