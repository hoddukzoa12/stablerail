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

const LABEL_CLASS = "mb-1 block text-sm text-text-secondary";

function Toggle({ label, value, onChange }: { label: string; value: boolean; onChange: (v: boolean) => void }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-sm text-text-secondary">{label}</span>
      <button
        type="button"
        onClick={() => onChange(!value)}
        className={`relative h-6 w-11 rounded-full transition-colors ${
          value ? "bg-success" : "bg-surface-3"
        }`}
      >
        <span
          className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-transform ${
            value ? "left-[22px]" : "left-0.5"
          }`}
        />
      </button>
    </div>
  );
}

export function PolicyForm({ policy, onSuccess }: PolicyFormProps) {
  const { execute, isSending, signature, error } = useUpdatePolicy();

  const [maxTradeAmount, setMaxTradeAmount] = useState(
    policy.maxTradeAmount.toFixed(0),
  );
  const [maxDailyVolume, setMaxDailyVolume] = useState(
    policy.maxDailyVolume.toFixed(0),
  );
  const [isActive, setIsActive] = useState(policy.isActive);

  // KYC/AML compliance fields
  const [kycRequired, setKycRequired] = useState(policy.kycRequired);
  const [maxRiskScore, setMaxRiskScore] = useState(String(policy.maxRiskScore));
  const [requireTravelRule, setRequireTravelRule] = useState(policy.requireTravelRule);
  const [travelRuleThreshold, setTravelRuleThreshold] = useState(
    policy.travelRuleThreshold > 0n
      ? String(Number(policy.travelRuleThreshold) / 1e6)
      : "",
  );

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    const tradeVal = parseFloat(maxTradeAmount);
    const dailyVal = parseFloat(maxDailyVolume);

    if (isNaN(tradeVal) || tradeVal <= 0) return;
    if (isNaN(dailyVal) || dailyVal <= 0) return;

    const tradeU64 = BigInt(Math.floor(tradeVal * 1e6));
    const dailyU64 = BigInt(Math.floor(dailyVal * 1e6));

    // Only send changed compliance fields
    const riskVal = parseInt(maxRiskScore, 10);
    const threshVal = parseFloat(travelRuleThreshold);

    try {
      await execute({
        maxTradeAmount: tradeU64,
        maxDailyVolume: dailyU64,
        isActive: isActive !== policy.isActive ? isActive : undefined,
        kycRequired: kycRequired !== policy.kycRequired ? kycRequired : undefined,
        maxRiskScore:
          !isNaN(riskVal) && riskVal !== policy.maxRiskScore
            ? Math.min(100, Math.max(0, riskVal))
            : undefined,
        requireTravelRule:
          requireTravelRule !== policy.requireTravelRule
            ? requireTravelRule
            : undefined,
        travelRuleThreshold:
          !isNaN(threshVal)
            ? BigInt(Math.floor(threshVal * 1e6))
            : travelRuleThreshold === "" && policy.travelRuleThreshold > 0n
              ? 0n
              : undefined,
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
        {/* ── Trade Limits ── */}
        <div>
          <label className={LABEL_CLASS}>Max Trade Amount (USD)</label>
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
          <label className={LABEL_CLASS}>Max Daily Volume (USD)</label>
          <input
            type="text"
            inputMode="decimal"
            value={maxDailyVolume}
            onChange={(e) => setMaxDailyVolume(e.target.value)}
            className={INPUT_CLASS}
            placeholder="500000000"
          />
        </div>

        <Toggle label="Policy Active" value={isActive} onChange={setIsActive} />

        {/* ── KYC/AML Compliance ── */}
        <div className="border-t border-border-default pt-4">
          <h4 className="mb-3 text-xs font-medium uppercase tracking-wider text-text-tertiary">
            KYC / AML Compliance
          </h4>

          <div className="space-y-3">
            <Toggle label="Require KYC" value={kycRequired} onChange={setKycRequired} />

            <div>
              <label className={LABEL_CLASS}>Max Risk Score (0–100)</label>
              <input
                type="text"
                inputMode="numeric"
                value={maxRiskScore}
                onChange={(e) => setMaxRiskScore(e.target.value)}
                className={INPUT_CLASS}
                placeholder="50"
              />
            </div>

            <Toggle
              label="Require Travel Rule"
              value={requireTravelRule}
              onChange={setRequireTravelRule}
            />

            {requireTravelRule && (
              <div>
                <label className={LABEL_CLASS}>Travel Rule Threshold (USD)</label>
                <input
                  type="text"
                  inputMode="decimal"
                  value={travelRuleThreshold}
                  onChange={(e) => setTravelRuleThreshold(e.target.value)}
                  className={INPUT_CLASS}
                  placeholder="3000"
                />
                <p className="mt-1 text-xs text-text-tertiary">
                  Settlements at or above this amount require Travel Rule data
                </p>
              </div>
            )}
          </div>
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
