"use client";

import { Loader2 } from "lucide-react";
import { useWalletConnection } from "@solana/react-hooks";
import { Button } from "../ui/button";
import type { TokenInfo } from "../../lib/tokens";

interface SwapButtonProps {
  tokenIn: TokenInfo;
  tokenOut: TokenInfo;
  amountIn: string;
  /** Input token balance in base units */
  balanceIn: bigint;
  hasQuote: boolean;
  isComputing: boolean;
  isSending: boolean;
  quoteError: string | null;
  onSwap: () => void;
  onConnect: () => void;
}

export function SwapButton({
  tokenIn,
  tokenOut,
  amountIn,
  balanceIn,
  hasQuote,
  isComputing,
  isSending,
  quoteError,
  onSwap,
  onConnect,
}: SwapButtonProps) {
  const { status } = useWalletConnection();

  // Not connected
  if (status !== "connected") {
    return (
      <Button variant="gradient" size="lg" className="w-full" onClick={onConnect}>
        Connect Wallet
      </Button>
    );
  }

  // Sending transaction
  if (isSending) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Swapping...
      </Button>
    );
  }

  // No amount entered
  const trimmed = amountIn.trim();
  if (!trimmed || parseFloat(trimmed) <= 0) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        Enter an amount
      </Button>
    );
  }

  // Insufficient balance
  const inputBaseUnits = BigInt(
    Math.floor(parseFloat(trimmed) * 10 ** tokenIn.decimals),
  );
  if (inputBaseUnits > balanceIn) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        Insufficient {tokenIn.symbol} balance
      </Button>
    );
  }

  // Computing quote
  if (isComputing) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        Fetching quote...
      </Button>
    );
  }

  // Quote error
  if (quoteError) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        {quoteError}
      </Button>
    );
  }

  // No quote available yet
  if (!hasQuote) {
    return (
      <Button variant="gradient" size="lg" className="w-full" disabled>
        Enter an amount
      </Button>
    );
  }

  // Ready to swap
  return (
    <Button variant="gradient" size="lg" className="w-full" onClick={onSwap}>
      Swap {tokenIn.symbol} → {tokenOut.symbol}
    </Button>
  );
}
