"use client";

import { useState, useCallback, useEffect, useMemo } from "react";
import { useWalletConnection } from "@solana/react-hooks";
import { type Address } from "@solana/kit";
import { Card } from "../ui/card";
import { TokenInputPanel } from "./token-input-panel";
import { SwapDirectionToggle } from "./swap-direction-toggle";
import { QuoteDetails } from "./quote-details";
import { SlippageSettings } from "./slippage-settings";
import { SwapButton } from "./swap-button";
import { TOKENS, type TokenInfo } from "../../lib/tokens";
import { deriveAta } from "../../lib/ata-utils";
import { usePoolState } from "../../hooks/usePoolState";
import { usePoolTicks } from "../../hooks/usePoolTicks";
import { useSwapQuote } from "../../hooks/useSwapQuote";
import { useTokenBalances } from "../../hooks/useTokenBalances";
import { useExecuteSwap } from "../../hooks/useExecuteSwap";
import type { TickData } from "../../lib/stablerail-math";

/** Default slippage: 0.5% = 50 bps */
const DEFAULT_SLIPPAGE_BPS = 50;

/** Read persisted slippage from localStorage */
function getInitialSlippage(): number {
  if (typeof window === "undefined") return DEFAULT_SLIPPAGE_BPS;
  const stored = localStorage.getItem("stablerail-slippage-bps");
  if (stored) {
    const parsed = parseInt(stored, 10);
    if (!isNaN(parsed) && parsed > 0 && parsed <= 5000) return parsed;
  }
  return DEFAULT_SLIPPAGE_BPS;
}

export function SwapCard() {
  const { wallet, connectors, connect } = useWalletConnection();

  // Token selection state
  const [tokenIn, setTokenIn] = useState<TokenInfo>(TOKENS[0]); // USDC
  const [tokenOut, setTokenOut] = useState<TokenInfo>(TOKENS[1]); // USDT
  const [amountIn, setAmountIn] = useState("");
  const [slippageBps, setSlippageBps] = useState(DEFAULT_SLIPPAGE_BPS);
  const [txResult, setTxResult] = useState<string | null>(null);

  // Load persisted slippage on mount
  useEffect(() => {
    setSlippageBps(getInitialSlippage());
  }, []);

  // Data hooks
  const { pool, isLoading: poolLoading } = usePoolState();
  const { ticks: rawTicks } = usePoolTicks(pool?.nAssets ?? 3);
  const { balances, refresh: refreshBalances } = useTokenBalances();

  // Convert TickInfo[] from usePoolTicks to TickData[] for the swap calculator.
  // Memoized to avoid creating new array references on every render.
  const tickData: TickData[] | undefined = useMemo(
    () =>
      rawTicks.length > 0
        ? rawTicks.map((t) => ({
            kRaw: t.kRaw,
            status: t.status,
            liquidityRaw: t.liquidityRaw,
            reservesRaw: t.reservesRaw,
          }))
        : undefined,
    [rawTicks],
  );

  const { quote, error: quoteError, isComputing } = useSwapQuote(
    pool,
    tokenIn.index,
    tokenOut.index,
    amountIn,
    tokenIn.decimals,
    tickData,
  );
  const { execute, isSending } = useExecuteSwap();

  // Computed output amount for display
  const amountOut =
    quote && quote.amountOutU64 > 0n
      ? (Number(quote.amountOutU64) / 10 ** tokenOut.decimals).toFixed(
          tokenOut.decimals > 4 ? 4 : tokenOut.decimals,
        )
      : "";

  // Direction toggle
  const handleToggle = useCallback(() => {
    setTokenIn(tokenOut);
    setTokenOut(tokenIn);
    setAmountIn("");
    setTxResult(null);
  }, [tokenIn, tokenOut]);

  // Token selection with auto-swap if same token
  const handleTokenInSelect = useCallback(
    (token: TokenInfo) => {
      if (token.index === tokenOut.index) {
        setTokenOut(tokenIn);
      }
      setTokenIn(token);
      setTxResult(null);
    },
    [tokenIn, tokenOut],
  );

  const handleTokenOutSelect = useCallback(
    (token: TokenInfo) => {
      if (token.index === tokenIn.index) {
        setTokenIn(tokenOut);
      }
      setTokenOut(token);
      setTxResult(null);
    },
    [tokenIn, tokenOut],
  );

  // Execute swap
  const handleSwap = useCallback(async () => {
    if (!quote || !wallet) return;

    const userAddress = wallet.account.address;

    try {
      const userAtaIn = await deriveAta(
        userAddress as Address,
        tokenIn.mint as Address,
      );
      const userAtaOut = await deriveAta(
        userAddress as Address,
        tokenOut.mint as Address,
      );

      // Compute min_amount_out with slippage
      const slippageMultiplier = 10000n - BigInt(slippageBps);
      const minAmountOut =
        (quote.amountOutU64 * slippageMultiplier) / 10000n;

      const inputBaseUnits = BigInt(
        Math.floor(parseFloat(amountIn) * 10 ** tokenIn.decimals),
      );

      const sig = await execute({
        tokenInIndex: tokenIn.index,
        tokenOutIndex: tokenOut.index,
        amountIn: inputBaseUnits,
        expectedAmountOut: quote.amountOutU64,
        minAmountOut,
        vaultIn: tokenIn.vault,
        vaultOut: tokenOut.vault,
        userAtaIn,
        userAtaOut,
        tickAddresses: rawTicks.map((t) => t.address),
      });

      setTxResult(sig);
      setAmountIn("");
      refreshBalances();
    } catch (err) {
      console.error("Swap failed:", err);
      setTxResult(null);
    }
  }, [
    quote,
    wallet,
    tokenIn,
    tokenOut,
    amountIn,
    slippageBps,
    execute,
    refreshBalances,
  ]);

  // Connect wallet handler
  const handleConnect = useCallback(() => {
    const phantom = connectors.find((c) =>
      c.name.toLowerCase().includes("phantom"),
    );
    if (phantom) {
      connect(phantom.id);
    } else if (connectors.length > 0) {
      connect(connectors[0].id);
    }
  }, [connectors, connect]);

  return (
    <Card variant="glass" className="w-full max-w-[440px] p-5">
      {/* Header */}
      <div className="mb-4 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-text-primary">Swap</h2>
        <SlippageSettings
          slippageBps={slippageBps}
          onSlippageChange={setSlippageBps}
        />
      </div>

      {/* Pool loading indicator */}
      {poolLoading && (
        <div className="mb-3 rounded-lg bg-surface-2 px-3 py-2 text-center text-xs text-text-tertiary">
          Loading pool data...
        </div>
      )}

      {/* From (input) panel */}
      <TokenInputPanel
        label="You pay"
        token={tokenIn}
        amount={amountIn}
        onAmountChange={setAmountIn}
        onTokenSelect={handleTokenInSelect}
        balance={balances[tokenIn.symbol] ?? 0n}
        disabledTokenIndex={tokenOut.index}
      />

      {/* Direction toggle */}
      <SwapDirectionToggle onToggle={handleToggle} />

      {/* To (output) panel */}
      <TokenInputPanel
        label="You receive"
        token={tokenOut}
        amount={amountOut}
        onTokenSelect={handleTokenOutSelect}
        balance={balances[tokenOut.symbol] ?? 0n}
        disabledTokenIndex={tokenIn.index}
        readOnly
      />

      {/* Quote details */}
      <div className="mt-3">
        <QuoteDetails
          quote={quote}
          tokenIn={tokenIn}
          tokenOut={tokenOut}
          slippageBps={slippageBps}
        />
      </div>

      {/* Swap button */}
      <div className="mt-4">
        <SwapButton
          tokenIn={tokenIn}
          tokenOut={tokenOut}
          amountIn={amountIn}
          balanceIn={balances[tokenIn.symbol] ?? 0n}
          hasQuote={!!quote}
          isComputing={isComputing}
          isSending={isSending}
          quoteError={quoteError}
          onSwap={handleSwap}
          onConnect={handleConnect}
        />
      </div>

      {/* Transaction result */}
      {txResult && (
        <div className="mt-3 rounded-lg bg-success/10 px-3 py-2 text-center text-xs text-success">
          Swap successful!{" "}
          <a
            href={`https://explorer.solana.com/tx/${txResult}?cluster=devnet`}
            target="_blank"
            rel="noopener noreferrer"
            className="underline underline-offset-2"
          >
            View on Explorer
          </a>
        </div>
      )}
    </Card>
  );
}
