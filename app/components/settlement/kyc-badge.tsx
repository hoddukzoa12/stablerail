"use client";

import { Badge } from "../ui/badge";
import type { KycEntryData, KycStatusType } from "../../lib/settlement-deserializer";

const STATUS_CONFIG: Record<KycStatusType, { variant: "success" | "warning" | "error" | "info"; label: string }> = {
  Verified: { variant: "success", label: "KYC Verified" },
  Pending: { variant: "warning", label: "KYC Pending" },
  Expired: { variant: "error", label: "KYC Expired" },
  Revoked: { variant: "error", label: "KYC Revoked" },
};

interface KycBadgeProps {
  entry: KycEntryData | null;
  compact?: boolean;
}

export function KycBadge({ entry, compact }: KycBadgeProps) {
  if (!entry) {
    return (
      <Badge variant="error" className="text-[10px]">
        No KYC
      </Badge>
    );
  }

  const config = STATUS_CONFIG[entry.kycStatus];
  const isExpired = entry.kycExpiry * 1000 < Date.now();
  const effectiveConfig = isExpired && entry.kycStatus === "Verified"
    ? STATUS_CONFIG.Expired
    : config;

  if (compact) {
    return (
      <Badge variant={effectiveConfig.variant} className="text-[10px]">
        {effectiveConfig.label}
      </Badge>
    );
  }

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        <Badge variant={effectiveConfig.variant} className="text-[10px]">
          {effectiveConfig.label}
        </Badge>
        {entry.amlCleared && (
          <Badge variant="success" className="text-[10px]">
            AML Clear
          </Badge>
        )}
        <Badge variant="info" className="text-[10px]">
          {entry.jurisdiction}
        </Badge>
      </div>
      <div className="flex items-center gap-3 text-[10px] text-text-tertiary">
        <span>
          Risk: <span className={`font-mono font-semibold ${entry.riskScore > 70 ? "text-error" : entry.riskScore > 40 ? "text-warning" : "text-success"}`}>
            {entry.riskScore}/100
          </span>
        </span>
        <span>
          Expires: {new Date(entry.kycExpiry * 1000).toLocaleDateString()}
        </span>
      </div>
    </div>
  );
}
