"use client";

/**
 * KYC Management — admin form for setting executor KYC/KYT/AML status.
 * Sends manage_kyc_entry instruction to the Orbital program.
 */

import { useState } from "react";
import { Button } from "../ui/button";
import { KycBadge } from "../settlement/kyc-badge";
import type { KycStatusType } from "../../lib/settlement-deserializer";

interface KycManagementProps {
  onSubmit: (params: {
    member: string;
    kycStatus: number;
    kycExpiry: number;
    riskScore: number;
    jurisdiction: string;
    amlCleared: boolean;
  }) => Promise<void>;
  isSending: boolean;
}

const KYC_STATUS_OPTIONS: { value: number; label: KycStatusType }[] = [
  { value: 0, label: "Pending" },
  { value: 1, label: "Verified" },
  { value: 2, label: "Expired" },
  { value: 3, label: "Revoked" },
];

export function KycManagement({ onSubmit, isSending }: KycManagementProps) {
  const [member, setMember] = useState("");
  const [kycStatus, setKycStatus] = useState(1); // Verified
  const [riskScore, setRiskScore] = useState(20);
  const [jurisdiction, setJurisdiction] = useState("US");
  const [amlCleared, setAmlCleared] = useState(true);
  const [expiryDays, setExpiryDays] = useState(365);

  const kycExpiry = Math.floor(Date.now() / 1000) + expiryDays * 86400;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!member) return;
    await onSubmit({
      member,
      kycStatus,
      kycExpiry,
      riskScore,
      jurisdiction,
      amlCleared,
    });
  }

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <h3 className="text-sm font-semibold text-text-primary">
        KYC Entry Management
      </h3>

      {/* Wallet address */}
      <div>
        <label className="mb-1 block text-xs text-text-secondary">
          Executor Wallet Address
        </label>
        <input
          type="text"
          value={member}
          onChange={(e) => setMember(e.target.value)}
          placeholder="Base58 address..."
          className="w-full rounded-lg bg-surface-2 px-3 py-2 text-sm text-text-primary outline-none placeholder:text-text-tertiary/40"
        />
      </div>

      {/* KYC Status */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-xs text-text-secondary">
            KYC Status
          </label>
          <select
            value={kycStatus}
            onChange={(e) => setKycStatus(Number(e.target.value))}
            className="w-full rounded-lg bg-surface-2 px-3 py-2 text-sm text-text-primary outline-none"
          >
            {KYC_STATUS_OPTIONS.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {opt.label}
              </option>
            ))}
          </select>
        </div>
        <div>
          <label className="mb-1 block text-xs text-text-secondary">
            Jurisdiction (ISO 3166)
          </label>
          <input
            type="text"
            maxLength={2}
            value={jurisdiction}
            onChange={(e) => setJurisdiction(e.target.value.toUpperCase())}
            className="w-full rounded-lg bg-surface-2 px-3 py-2 text-sm font-mono text-text-primary outline-none"
          />
        </div>
      </div>

      {/* Risk Score & Expiry */}
      <div className="grid grid-cols-2 gap-3">
        <div>
          <label className="mb-1 block text-xs text-text-secondary">
            Risk Score (0-100)
          </label>
          <input
            type="number"
            min={0}
            max={100}
            value={riskScore}
            onChange={(e) => setRiskScore(Number(e.target.value))}
            className="w-full rounded-lg bg-surface-2 px-3 py-2 text-sm font-mono text-text-primary outline-none"
          />
        </div>
        <div>
          <label className="mb-1 block text-xs text-text-secondary">
            Expires in (days)
          </label>
          <input
            type="number"
            min={1}
            max={3650}
            value={expiryDays}
            onChange={(e) => setExpiryDays(Number(e.target.value))}
            className="w-full rounded-lg bg-surface-2 px-3 py-2 text-sm font-mono text-text-primary outline-none"
          />
        </div>
      </div>

      {/* AML Toggle */}
      <label className="flex cursor-pointer items-center gap-2">
        <input
          type="checkbox"
          checked={amlCleared}
          onChange={(e) => setAmlCleared(e.target.checked)}
          className="h-4 w-4 rounded"
        />
        <span className="text-xs text-text-secondary">AML Screening Cleared</span>
      </label>

      {/* Preview */}
      <div className="rounded-lg bg-surface-2 p-3">
        <div className="mb-1 text-[10px] text-text-tertiary">Preview</div>
        <KycBadge
          entry={{
            bump: 0,
            policy: "",
            address: member || "...",
            kycStatus: KYC_STATUS_OPTIONS.find((o) => o.value === kycStatus)?.label ?? "Pending",
            kycExpiry,
            riskScore,
            jurisdiction,
            amlCleared,
            updatedAt: Math.floor(Date.now() / 1000),
          }}
        />
      </div>

      <Button
        type="submit"
        disabled={!member || isSending}
        className="w-full"
      >
        {isSending ? "Submitting..." : "Set KYC Entry"}
      </Button>
    </form>
  );
}
