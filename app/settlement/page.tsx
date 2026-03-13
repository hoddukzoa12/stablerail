"use client";

import { useWalletConnection } from "@solana/react-hooks";
import { usePolicy } from "../hooks/usePolicy";
import { useAllowlist } from "../hooks/useAllowlist";
import { useAuditTrail } from "../hooks/useAuditTrail";
import { useTokenBalances } from "../hooks/useTokenBalances";
import { SettlementForm } from "../components/settlement/settlement-form";
import { AuditTrailTable } from "../components/settlement/audit-trail-table";
import { Building2, ShieldAlert } from "lucide-react";

export default function SettlementPage() {
  const { wallet, status } = useWalletConnection();
  const { policy, isLoading: policyLoading, refresh: refreshPolicy } = usePolicy();
  const { isAllowed, isLoading: allowlistLoading } = useAllowlist();
  const { settlements, isLoading: auditLoading, refresh: refreshAudit } = useAuditTrail();
  const { balances: tokenBalances, refresh: refreshBalances } = useTokenBalances();

  const isConnected = status === "connected" && wallet;
  const walletAddress = wallet?.account.address.toString() ?? "";
  const walletAllowed = isAllowed(walletAddress);

  function handleSuccess(): void {
    refreshPolicy();
    refreshAudit();
    refreshBalances();
  }

  if (policyLoading || allowlistLoading) {
    return (
      <div className="mx-auto max-w-5xl px-4 pt-24">
        <div className="flex items-center justify-center py-20">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-brand-primary border-t-transparent" />
        </div>
      </div>
    );
  }

  if (!isConnected) {
    return (
      <div className="mx-auto max-w-5xl px-4 pt-24">
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <Building2 className="mb-4 h-12 w-12 text-text-tertiary" />
          <h2 className="text-lg font-semibold text-text-primary">
            Connect Wallet
          </h2>
          <p className="mt-2 text-sm text-text-secondary">
            Connect your wallet to access the settlement terminal.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-5xl px-4 pt-24 pb-12">
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-text-primary">
          Settlement Terminal
        </h1>
        <p className="mt-1 text-sm text-text-secondary">
          Execute institutional settlements with policy compliance
        </p>
      </div>

      {!walletAllowed && (
        <div className="mb-6 flex items-center gap-3 rounded-xl bg-warning/10 px-5 py-4">
          <ShieldAlert className="h-5 w-5 flex-shrink-0 text-warning" />
          <div>
            <p className="text-sm font-medium text-warning">Not Authorized</p>
            <p className="mt-0.5 text-xs text-text-secondary">
              Your wallet is not in the allowlist. Contact the pool admin to request access.
            </p>
          </div>
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-5">
        <div className="lg:col-span-2">
          {walletAllowed && policy ? (
            <SettlementForm
              policy={policy}
              tokenBalances={tokenBalances}
              onSuccess={handleSuccess}
            />
          ) : (
            <div className="rounded-xl border border-border-default bg-surface-1 p-6 text-center">
              <Building2 className="mx-auto mb-3 h-10 w-10 text-text-tertiary" />
              <p className="text-sm text-text-secondary">
                {!policy
                  ? "No policy configured for this pool."
                  : "Settlement execution requires allowlist membership."}
              </p>
            </div>
          )}
        </div>

        <div className="lg:col-span-3">
          <AuditTrailTable settlements={settlements} isLoading={auditLoading} />
        </div>
      </div>
    </div>
  );
}
