"use client";

import { useState } from "react";
import { Card } from "../ui/card";
import { Button } from "../ui/button";
import { TxNotification } from "../ui/tx-notification";
import { useUpdatePolicy } from "../../hooks/useUpdatePolicy";
import type { PolicyStateData } from "../../lib/settlement-deserializer";

interface PolicyFormProps {
  policy: PolicyStateData;
  onSuccess: () => void;
}

const INPUT_CLASS =
  "w-full rounded-lg border border-border-default bg-surface-2 px-3 py-2 font-mono text-sm text-text-primary outline-none focus:border-brand-primary";

export function PolicyForm({ policy, onSuccess }: PolicyFormProps) {
  const { execute, isSending, signature, error } = useUpdatePolicy();

  const [maxTradeAmount, setMaxTradeAmount] = useState(
    policy.maxTradeAmount.toFixed(0),
  );
  const [maxDailyVolume, setMaxDailyVolume] = useState(
    policy.maxDailyVolume.toFixed(0),
  );
  const [isActive, setIsActive] = useState(policy.isActive);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    const tradeVal = parseFloat(maxTradeAmount);
    const dailyVal = parseFloat(maxDailyVolume);

    if (isNaN(tradeVal) || tradeVal <= 0) return;
    if (isNaN(dailyVal) || dailyVal <= 0) return;

    const tradeU64 = BigInt(Math.floor(tradeVal * 1e6));
    const dailyU64 = BigInt(Math.floor(dailyVal * 1e6));

    try {
      await execute({
        maxTradeAmount: tradeU64,
        maxDailyVolume: dailyU64,
        isActive: isActive !== policy.isActive ? isActive : undefined,
      });
      onSuccess();
    } catch {
      // error is already set in hook
    }
  };

  return (
    <Card variant="glass">
      <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
        Update Policy
      </h3>

      <form onSubmit={handleSubmit} className="mt-4 space-y-4">
        <div>
          <label className="mb-1 block text-sm text-text-secondary">
            Max Trade Amount (USD)
          </label>
          <input
            type="text"
            inputMode="decimal"
            value={maxTradeAmount}
            onChange={(e) => setMaxTradeAmount(e.target.value)}
            className={INPUT_CLASS}
            placeholder="50000000"
          />
        </div>

        <div>
          <label className="mb-1 block text-sm text-text-secondary">
            Max Daily Volume (USD)
          </label>
          <input
            type="text"
            inputMode="decimal"
            value={maxDailyVolume}
            onChange={(e) => setMaxDailyVolume(e.target.value)}
            className={INPUT_CLASS}
            placeholder="500000000"
          />
        </div>

        <div className="flex items-center justify-between">
          <span className="text-sm text-text-secondary">Policy Active</span>
          <button
            type="button"
            onClick={() => setIsActive(!isActive)}
            className={`relative h-6 w-11 rounded-full transition-colors ${
              isActive ? "bg-success" : "bg-surface-3"
            }`}
          >
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
                isActive ? "left-[22px]" : "left-0.5"
              }`}
            />
          </button>
        </div>

        <Button
          type="submit"
          variant="primary"
          size="md"
          className="w-full"
          disabled={isSending}
        >
          {isSending ? "Updating..." : "Update Policy"}
        </Button>
      </form>

      <TxNotification
        error={error}
        txSignature={signature}
        successLabel="Policy updated!"
      />
    </Card>
  );
}
