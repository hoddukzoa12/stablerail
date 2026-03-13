"use client";

import { useState } from "react";
import { Card } from "../ui/card";
import { Button } from "../ui/button";
import { TxNotification } from "../ui/tx-notification";
import { useManageAllowlist } from "../../hooks/useManageAllowlist";

interface AllowlistManagerProps {
  onSuccess: () => void;
}

function isValidSolanaAddress(addr: string): boolean {
  if (addr.length < 32 || addr.length > 44) return false;
  return /^[1-9A-HJ-NP-Za-km-z]+$/.test(addr);
}

export function AllowlistManager({ onSuccess }: AllowlistManagerProps) {
  const { execute, isSending, signature, error } = useManageAllowlist();
  const [address, setAddress] = useState("");
  const [validationError, setValidationError] = useState<string | null>(null);

  const handleAdd = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!isValidSolanaAddress(address)) {
      setValidationError("Invalid Solana address format");
      return;
    }
    setValidationError(null);

    try {
      await execute({ action: "Add", address });
      setAddress("");
      onSuccess();
    } catch {
      // error handled by hook
    }
  };

  return (
    <Card variant="glass">
      <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
        Add Member
      </h3>

      <form onSubmit={handleAdd} className="mt-4 space-y-3">
        <div>
          <input
            type="text"
            value={address}
            onChange={(e) => {
              setAddress(e.target.value);
              setValidationError(null);
            }}
            placeholder="Solana wallet address"
            className="w-full rounded-lg border border-border-default bg-surface-2 px-3 py-2 font-mono text-sm text-text-primary outline-none focus:border-brand-primary"
          />
          {validationError && (
            <p className="mt-1 text-xs text-error">{validationError}</p>
          )}
        </div>

        <Button
          type="submit"
          variant="primary"
          size="md"
          className="w-full"
          disabled={isSending || !address}
        >
          {isSending ? "Adding..." : "Add to Allowlist"}
        </Button>
      </form>

      <TxNotification
        error={error}
        txSignature={signature}
        successLabel="Address added!"
      />
    </Card>
  );
}
