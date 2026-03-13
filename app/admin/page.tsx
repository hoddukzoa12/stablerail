"use client";

import { useWalletConnection } from "@solana/react-hooks";
import { usePolicy } from "../hooks/usePolicy";
import { useAllowlist } from "../hooks/useAllowlist";
import { PolicyStatusCard } from "../components/admin/policy-status-card";
import { PolicyForm } from "../components/admin/policy-form";
import { AllowlistTable } from "../components/admin/allowlist-table";
import { AllowlistManager } from "../components/admin/allowlist-manager";
import { Shield, AlertTriangle } from "lucide-react";

const PAGE_WRAPPER = "mx-auto max-w-5xl px-4 pt-24";

function StatusScreen({
  icon,
  title,
  description,
}: {
  icon: React.ReactNode;
  title: string;
  description: string;
}) {
  return (
    <div className={PAGE_WRAPPER}>
      <div className="flex flex-col items-center justify-center py-20 text-center">
        {icon}
        <h2 className="text-lg font-semibold text-text-primary">{title}</h2>
        <p className="mt-2 text-sm text-text-secondary">{description}</p>
      </div>
    </div>
  );
}

export default function AdminPage() {
  const { wallet, status } = useWalletConnection();
  const { policy, isLoading: policyLoading, refresh: refreshPolicy } = usePolicy();
  const { addresses, isLoading: allowlistLoading, refresh: refreshAllowlist } = useAllowlist();

  const isConnected = status === "connected" && wallet;
  const walletAddress = wallet?.account.address.toString() ?? "";
  const isAuthority = Boolean(policy && walletAddress === policy.authority);

  if (policyLoading || allowlistLoading) {
    return (
      <div className={PAGE_WRAPPER}>
        <div className="flex items-center justify-center py-20">
          <div className="h-8 w-8 animate-spin rounded-full border-2 border-brand-primary border-t-transparent" />
        </div>
      </div>
    );
  }

  if (!isConnected) {
    return (
      <StatusScreen
        icon={<Shield className="mb-4 h-12 w-12 text-text-tertiary" />}
        title="Connect Wallet"
        description="Connect your wallet to access the admin panel."
      />
    );
  }

  if (!policy) {
    return (
      <StatusScreen
        icon={<AlertTriangle className="mb-4 h-12 w-12 text-warning" />}
        title="Policy Not Found"
        description="No policy account exists for this pool."
      />
    );
  }

  return (
    <div className={`${PAGE_WRAPPER} pb-12`}>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-text-primary">Admin Panel</h1>
        <p className="mt-1 text-sm text-text-secondary">
          Manage settlement policy and allowlist
        </p>
        {!isAuthority && (
          <div className="mt-3 rounded-lg bg-warning/10 px-4 py-2 text-sm text-warning">
            Read-only mode — your wallet is not the policy authority.
          </div>
        )}
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <div className="space-y-6">
          <PolicyStatusCard policy={policy} />
          {isAuthority && (
            <PolicyForm policy={policy} onSuccess={refreshPolicy} />
          )}
        </div>

        <div className="space-y-6">
          {isAuthority && (
            <AllowlistManager onSuccess={refreshAllowlist} />
          )}
          <AllowlistTable
            addresses={addresses}
            isAuthority={isAuthority}
            onSuccess={refreshAllowlist}
          />
        </div>
      </div>
    </div>
  );
}
