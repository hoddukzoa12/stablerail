"use client";

import { Card } from "../ui/card";
import { formatUsd } from "../../lib/format-utils";
import type { PolicyStateData } from "../../lib/settlement-deserializer";
import { CheckCircle2, XCircle } from "lucide-react";

interface PolicyCompliancePreviewProps {
  policy: PolicyStateData;
  /** Amount in display units (e.g. 1000 = $1000) */
  amount: number;
}

export function PolicyCompliancePreview({
  policy,
  amount,
}: PolicyCompliancePreviewProps) {
  if (amount <= 0) return null;

  const withinTradeLimit = amount <= policy.maxTradeAmount;
  const projectedDailyVolume = policy.currentDailyVolume + amount;
  const withinDailyLimit = projectedDailyVolume <= policy.maxDailyVolume;
  const allClear = withinTradeLimit && withinDailyLimit && policy.isActive;

  return (
    <Card variant="default" className="!p-4">
      <h4 className="mb-3 text-xs font-medium uppercase tracking-wider text-text-tertiary">
        Policy Compliance
      </h4>

      <div className="space-y-2">
        <ComplianceRow
          label="Policy Active"
          ok={policy.isActive}
          detail={policy.isActive ? "Yes" : "Inactive"}
        />
        <ComplianceRow
          label="Per-tx Limit"
          ok={withinTradeLimit}
          detail={`${formatUsd(amount)} / ${formatUsd(policy.maxTradeAmount)}`}
        />
        <ComplianceRow
          label="Daily Volume"
          ok={withinDailyLimit}
          detail={`${formatUsd(projectedDailyVolume)} / ${formatUsd(policy.maxDailyVolume)}`}
        />
      </div>

      <div
        className={`mt-3 rounded-lg px-3 py-1.5 text-center text-xs font-medium ${
          allClear
            ? "bg-success/10 text-success"
            : "bg-error/10 text-error"
        }`}
      >
        {allClear ? "All checks passed" : "Settlement would be rejected"}
      </div>
    </Card>
  );
}

function ComplianceRow({
  label,
  ok,
  detail,
}: {
  label: string;
  ok: boolean;
  detail: string;
}) {
  const Icon = ok ? CheckCircle2 : XCircle;
  const iconColor = ok ? "text-success" : "text-error";

  return (
    <div className="flex items-center justify-between">
      <div className="flex items-center gap-2">
        <Icon className={`h-4 w-4 ${iconColor}`} />
        <span className="text-sm text-text-secondary">{label}</span>
      </div>
      <span
        className={`font-mono text-xs ${ok ? "text-text-secondary" : "text-error"}`}
      >
        {detail}
      </span>
    </div>
  );
}
