"use client";

import { KycBadge } from "../settlement/kyc-badge";
import type { KycEntryData } from "../../lib/settlement-deserializer";

interface KycEntryTableProps {
  entries: KycEntryData[];
  isLoading: boolean;
}

function truncateAddress(addr: string): string {
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 4)}...${addr.slice(-4)}`;
}

export function KycEntryTable({ entries, isLoading }: KycEntryTableProps) {
  if (isLoading && entries.length === 0) {
    return (
      <div className="rounded-xl border border-border-default bg-surface-1 p-6">
        <h3 className="mb-3 text-sm font-semibold text-text-primary">
          KYC Registry
        </h3>
        <div className="flex justify-center py-4">
          <div className="h-5 w-5 animate-spin rounded-full border-2 border-brand-primary border-t-transparent" />
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-xl border border-border-default bg-surface-1 p-6">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-text-primary">
          KYC Registry
        </h3>
        <span className="text-[10px] text-text-tertiary">
          {entries.length} {entries.length === 1 ? "entry" : "entries"}
        </span>
      </div>

      {entries.length === 0 ? (
        <p className="py-4 text-center text-xs text-text-tertiary">
          No KYC entries found. Use the form above to register executors.
        </p>
      ) : (
        <div className="space-y-2">
          {entries.map((entry) => (
            <div
              key={entry.address}
              className="rounded-lg bg-surface-2 p-3"
            >
              <div className="mb-2 flex items-center justify-between">
                <span
                  className="font-mono text-xs text-text-primary"
                  title={entry.address}
                >
                  {truncateAddress(entry.address)}
                </span>
                <KycBadge entry={entry} compact />
              </div>
              <div className="flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-text-tertiary">
                <span>
                  Risk:{" "}
                  <span
                    className={`font-mono font-semibold ${
                      entry.riskScore > 70
                        ? "text-error"
                        : entry.riskScore > 40
                          ? "text-warning"
                          : "text-success"
                    }`}
                  >
                    {entry.riskScore}/100
                  </span>
                </span>
                <span>
                  Jurisdiction:{" "}
                  <span className="font-mono">{entry.jurisdiction}</span>
                </span>
                <span>
                  AML:{" "}
                  <span
                    className={
                      entry.amlCleared ? "text-success" : "text-error"
                    }
                  >
                    {entry.amlCleared ? "Cleared" : "Not cleared"}
                  </span>
                </span>
                <span>
                  Expires:{" "}
                  <span className="font-mono">
                    {new Date(entry.kycExpiry * 1000).toLocaleDateString()}
                  </span>
                </span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
