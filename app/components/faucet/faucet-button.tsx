"use client";

import { useFaucet } from "../../hooks/useFaucet";
import { useWalletConnection } from "@solana/react-hooks";
import { explorerUrl } from "../../lib/format-utils";
import { Droplets } from "lucide-react";

export function FaucetButton() {
  const { wallet, status } = useWalletConnection();
  const { requestTokens, isLoading, signature, error } = useFaucet();

  if (status !== "connected" || !wallet) return null;

  return (
    <div className="relative">
      <button
        onClick={requestTokens}
        disabled={isLoading}
        className="flex items-center gap-1.5 rounded-full bg-surface-2 px-3 py-1.5 text-sm font-medium text-text-secondary transition-colors hover:bg-surface-3 hover:text-text-primary disabled:opacity-50 cursor-pointer"
        title="Get 10K test tokens (USDC, USDT, PYUSD)"
      >
        <Droplets className="h-3.5 w-3.5" />
        {isLoading ? "Minting..." : "Faucet"}
      </button>

      {(signature || error) && (
        <div className="absolute right-0 top-full z-50 mt-2 w-64 rounded-lg border border-border-default bg-surface-1 p-3 shadow-lg">
          {error ? (
            <p className="text-xs text-error">{error}</p>
          ) : (
            <div className="text-xs">
              <p className="text-success">10K tokens minted!</p>
              <a
                href={explorerUrl("tx", signature!)}
                target="_blank"
                rel="noopener noreferrer"
                className="mt-1 inline-block text-brand-primary hover:underline"
              >
                View on Explorer
              </a>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
