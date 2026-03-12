"use client";

import { type TokenInfo } from "../../lib/tokens";
import { TokenSelector } from "./token-selector";

interface TokenInputPanelProps {
  label: string;
  token: TokenInfo;
  amount: string;
  onAmountChange?: (value: string) => void;
  onTokenSelect: (token: TokenInfo) => void;
  /** Balance in base units (bigint) */
  balance: bigint;
  /** Token index to disable in the selector */
  disabledTokenIndex?: number;
  /** Read-only output mode */
  readOnly?: boolean;
}

/** Format base units to human-readable with commas */
function formatBalance(baseUnits: bigint, decimals: number): string {
  const divisor = 10 ** decimals;
  const whole = Number(baseUnits) / divisor;
  return whole.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: decimals,
  });
}

export function TokenInputPanel({
  label,
  token,
  amount,
  onAmountChange,
  onTokenSelect,
  balance,
  disabledTokenIndex,
  readOnly = false,
}: TokenInputPanelProps) {
  const formattedBalance = formatBalance(balance, token.decimals);

  const handleMax = () => {
    if (readOnly || !onAmountChange) return;
    const maxAmount = Number(balance) / 10 ** token.decimals;
    onAmountChange(maxAmount.toString());
  };

  return (
    <div className="rounded-xl bg-surface-2 p-4">
      {/* Top row: label + balance */}
      <div className="mb-2 flex items-center justify-between">
        <span className="text-xs font-medium text-text-tertiary">{label}</span>
        <span className="text-xs text-text-tertiary">
          Balance: {formattedBalance}
        </span>
      </div>

      {/* Bottom row: input + token selector */}
      <div className="flex items-center gap-3">
        <input
          type="text"
          inputMode="decimal"
          placeholder="0.00"
          value={amount}
          readOnly={readOnly}
          onChange={(e) => {
            if (readOnly || !onAmountChange) return;
            // Only allow numeric input with decimals
            const val = e.target.value;
            if (/^[0-9]*\.?[0-9]*$/.test(val)) {
              onAmountChange(val);
            }
          }}
          className={`min-w-0 flex-1 bg-transparent font-mono text-2xl font-semibold text-text-primary outline-none placeholder:text-text-tertiary/50 ${
            readOnly ? "cursor-default" : ""
          }`}
        />

        <div className="flex items-center gap-2">
          {!readOnly && balance > 0n && (
            <button
              type="button"
              onClick={handleMax}
              className="rounded-md bg-brand-primary/10 px-2 py-0.5 text-[10px] font-semibold uppercase text-brand-primary transition-colors hover:bg-brand-primary/20 cursor-pointer"
            >
              MAX
            </button>
          )}
          <TokenSelector
            selected={token}
            onSelect={onTokenSelect}
            disabledIndex={disabledTokenIndex}
          />
        </div>
      </div>
    </div>
  );
}
