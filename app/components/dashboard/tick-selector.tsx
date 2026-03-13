"use client";

/**
 * Tick Selector — choose Full Range or Concentrated (Tick) LP mode.
 *
 * When "Concentrated" is selected, shows:
 *   - Existing ticks to select from
 *   - Or a k-value input to create a new tick
 *   - Real-time preview of tick properties (x_min, x_max, depeg_price, etc.)
 */

import { useState, useMemo } from "react";
import { Badge } from "../ui/badge";
import { q6464ToNumber } from "../../lib/format-utils";
import { computeTickPreview } from "../../lib/tick-math";
import type { TickInfo } from "../../lib/tick-deserializer";
import type { PoolState } from "../../lib/stablerail-math";

export type LiquidityMode = "full-range" | "concentrated";

export interface TickSelection {
  mode: LiquidityMode;
  /** Selected existing tick address (if choosing from list) */
  tickAddress?: string;
  /** k_raw bigint for new tick creation */
  kRaw?: bigint;
}

interface TickSelectorProps {
  pool: PoolState;
  ticks: TickInfo[];
  ticksLoading: boolean;
  selection: TickSelection;
  onChange: (selection: TickSelection) => void;
}

/** Format a number with 4 decimal places. */
function fmt(n: number): string {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });
}

export function TickSelector({
  pool,
  ticks,
  ticksLoading,
  selection,
  onChange,
}: TickSelectorProps) {
  const [kInput, setKInput] = useState("");

  const radius = q6464ToNumber(pool.radius.raw);
  const n = pool.nAssets;

  // Compute preview for the k input value
  const kPreview = useMemo(() => {
    const k = parseFloat(kInput);
    if (!kInput || isNaN(k)) return null;
    return computeTickPreview(k, radius, n);
  }, [kInput, radius, n]);

  // Convert k number to Q64.64 raw bigint
  const kRawFromInput = useMemo(() => {
    const k = parseFloat(kInput);
    if (!kInput || isNaN(k)) return null;
    // Q64.64: raw = k * 2^64
    const SCALE = 1n << 64n;
    const negative = k < 0;
    const abs = Math.abs(k);
    const intPart = BigInt(Math.floor(abs));
    const fracPart = abs - Number(intPart);
    const fracScaled = BigInt(Math.round(fracPart * Number(SCALE)));
    let raw = (intPart << 64n) + fracScaled;
    if (negative) raw = -raw;
    return raw;
  }, [kInput]);

  const interiorTicks = ticks.filter((t) => t.status === "Interior");

  return (
    <div className="space-y-3">
      {/* Mode toggle */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => onChange({ mode: "full-range" })}
          className={`flex-1 cursor-pointer rounded-lg px-3 py-2.5 text-xs font-medium transition-all ${
            selection.mode === "full-range"
              ? "bg-brand-primary/20 text-brand-primary ring-1 ring-brand-primary/40"
              : "bg-surface-2 text-text-secondary hover:bg-surface-3"
          }`}
        >
          <div className="font-semibold">Full Range</div>
          <div className="mt-0.5 text-[10px] opacity-70">
            Earn on all trades
          </div>
        </button>
        <button
          type="button"
          onClick={() => onChange({ mode: "concentrated" })}
          className={`flex-1 cursor-pointer rounded-lg px-3 py-2.5 text-xs font-medium transition-all ${
            selection.mode === "concentrated"
              ? "bg-accent-blue/20 text-accent-blue ring-1 ring-accent-blue/40"
              : "bg-surface-2 text-text-secondary hover:bg-surface-3"
          }`}
        >
          <div className="font-semibold">Concentrated</div>
          <div className="mt-0.5 text-[10px] opacity-70">
            Higher efficiency
          </div>
        </button>
      </div>

      {/* Concentrated mode details */}
      {selection.mode === "concentrated" && (
        <div className="space-y-3">
          {/* Existing ticks list */}
          {ticksLoading ? (
            <div className="rounded-lg bg-surface-2 p-3 text-center text-xs text-text-tertiary">
              Loading ticks...
            </div>
          ) : interiorTicks.length > 0 ? (
            <div className="space-y-1.5">
              <div className="text-[11px] font-medium text-text-secondary">
                Select existing tick
              </div>
              {interiorTicks.map((tick) => (
                <button
                  key={tick.address}
                  type="button"
                  onClick={() =>
                    onChange({
                      mode: "concentrated",
                      tickAddress: tick.address,
                    })
                  }
                  className={`w-full cursor-pointer rounded-lg p-2.5 text-left transition-all ${
                    selection.tickAddress === tick.address
                      ? "bg-accent-blue/15 ring-1 ring-accent-blue/40"
                      : "bg-surface-2 hover:bg-surface-3"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-xs text-text-primary">
                        k = {fmt(tick.kDisplay)}
                      </span>
                      <Badge
                        variant={
                          tick.status === "Interior" ? "success" : "warning"
                        }
                      >
                        {tick.status}
                      </Badge>
                    </div>
                    <span className="text-[10px] text-text-tertiary">
                      {fmt(tick.capitalEfficiency)}× eff.
                    </span>
                  </div>
                  <div className="mt-1 flex gap-3 text-[10px] text-text-tertiary">
                    <span>
                      Range: {fmt(tick.xMin)} – {fmt(tick.xMax)}
                    </span>
                    <span>Depeg: {fmt(tick.depegPrice)}</span>
                  </div>
                </button>
              ))}
            </div>
          ) : null}

          {/* Divider */}
          {interiorTicks.length > 0 && (
            <div className="flex items-center gap-2">
              <div className="h-px flex-1 bg-border-default" />
              <span className="text-[10px] text-text-tertiary">or</span>
              <div className="h-px flex-1 bg-border-default" />
            </div>
          )}

          {/* New tick creation */}
          <div className="space-y-2">
            <div className="text-[11px] font-medium text-text-secondary">
              Create new tick
            </div>
            <div className="rounded-lg bg-surface-2 p-3">
              <div className="flex items-center gap-2">
                <label className="text-xs text-text-secondary">k =</label>
                <input
                  type="text"
                  inputMode="decimal"
                  placeholder={fmt(radius * (Math.sqrt(n) - 1) * 1.1)}
                  value={kInput}
                  onChange={(e) => {
                    const v = e.target.value;
                    if (/^[0-9]*\.?[0-9]*$/.test(v)) {
                      setKInput(v);
                      if (v && !isNaN(parseFloat(v)) && kRawFromInput) {
                        onChange({
                          mode: "concentrated",
                          kRaw: kRawFromInput,
                        });
                      }
                    }
                  }}
                  className="min-w-0 flex-1 bg-transparent font-mono text-sm text-text-primary outline-none placeholder:text-text-tertiary/40"
                />
              </div>

              {/* Preview */}
              {kPreview && (
                <div className="mt-2.5 grid grid-cols-2 gap-x-4 gap-y-1 border-t border-border-default pt-2.5 text-[10px]">
                  <div className="flex justify-between">
                    <span className="text-text-tertiary">x_min</span>
                    <span className="font-mono text-text-secondary">
                      {fmt(kPreview.xMin)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-text-tertiary">x_max</span>
                    <span className="font-mono text-text-secondary">
                      {fmt(kPreview.xMax)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-text-tertiary">Depeg price</span>
                    <span className="font-mono text-text-secondary">
                      {fmt(kPreview.depegPrice)}
                    </span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-text-tertiary">Efficiency</span>
                    <span className="font-mono text-accent-blue">
                      {fmt(kPreview.capitalEfficiency)}×
                    </span>
                  </div>
                  {/* k position bar */}
                  <div className="col-span-2 mt-1">
                    <div className="mb-0.5 flex justify-between text-[9px] text-text-tertiary">
                      <span>k_min ({fmt(kPreview.kMin)})</span>
                      <span>k_max ({fmt(kPreview.kMax)})</span>
                    </div>
                    <div className="h-1.5 rounded-full bg-surface-3">
                      <div
                        className="h-1.5 rounded-full bg-accent-blue transition-all"
                        style={{
                          width: `${Math.max(0, Math.min(100, kPreview.kPercent))}%`,
                        }}
                      />
                    </div>
                  </div>
                </div>
              )}

              {/* Invalid k warning */}
              {kInput && !kPreview && (
                <p className="mt-2 text-[10px] text-error">
                  k must be between k_min and k_max for this pool
                </p>
              )}
            </div>
          </div>

          {/* Info banner */}
          <div className="rounded-lg bg-accent-blue/8 px-3 py-2 text-[10px] text-accent-blue/80">
            Concentrated liquidity earns more fees when the pool trades near
            your tick's range, but earns nothing when trading outside it.
            Narrower range = higher efficiency = higher risk.
          </div>
        </div>
      )}
    </div>
  );
}
