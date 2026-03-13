"use client";

import { Card } from "../ui/card";
import { Badge } from "../ui/badge";
import { Button } from "../ui/button";
import { TxNotification } from "../ui/tx-notification";
import { truncateAddress, explorerUrl } from "../../lib/format-utils";
import { useManageAllowlist } from "../../hooks/useManageAllowlist";

interface AllowlistTableProps {
  addresses: string[];
  isAuthority: boolean;
  onSuccess: () => void;
}

export function AllowlistTable({ addresses, isAuthority, onSuccess }: AllowlistTableProps) {
  const { execute, isSending, signature, error } = useManageAllowlist();

  const handleRemove = async (address: string) => {
    try {
      await execute({ action: "Remove", address });
      onSuccess();
    } catch {
      // error handled by hook
    }
  };

  return (
    <Card variant="glass">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
          Allowlist
        </h3>
        <Badge variant="info">{addresses.length} members</Badge>
      </div>

      <div className="mt-4">
        {addresses.length === 0 ? (
          <p className="py-4 text-center text-sm text-text-tertiary">
            No addresses in allowlist
          </p>
        ) : (
          <div className="space-y-2">
            {addresses.map((addr) => (
              <div
                key={addr}
                className="flex items-center justify-between rounded-lg bg-surface-2 px-3 py-2"
              >
                <a
                  href={explorerUrl("address", addr)}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="font-mono text-sm text-brand-primary hover:underline"
                >
                  {truncateAddress(addr, 6, 6)}
                </a>
                {isAuthority && (
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => handleRemove(addr)}
                    disabled={isSending}
                    className="text-error hover:text-error"
                  >
                    Remove
                  </Button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      <TxNotification
        error={error}
        txSignature={signature}
        successLabel="Allowlist updated!"
      />
    </Card>
  );
}
