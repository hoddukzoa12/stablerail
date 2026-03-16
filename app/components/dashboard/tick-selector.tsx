"use client";

/**
 * Tick Selector — choose Full Range or Concentrated LP mode.
 *
 * Concentrated mode offers:
 *   - Preset concentration levels (Low / Medium / High)
 *   - Custom k-value input with clear min/max bounds
 *   - Real-time preview of tick properties (x_min, x_max, depeg_price, etc.)
 *   - Auto-selects existing tick if one matches, otherwise creates new
 */

import { useState, useMemo } from "react";
import { Badge } from "../ui/badge";
import { q6464ToNumber } from "../../lib/format-utils";
import {
  computeTickPreview,
  computeKMin,
  computeKMax,
} from "../../lib/tick-math";
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

// ── Preset concentration levels ──

type PresetLevel = "low" | "medium" | "high" | "custom";

interface PresetConfig {
  label: string;
  description: string;
  /** Position between k_min and k_max (0 = widest, 1 = narrowest) */
  kPercent: number;
  color: string;
  activeColor: string;
}

const PRESETS: Record<Exclude<PresetLevel, "custom">, PresetConfig> = {
  low: {
    label: "Safe",
    description: "Covers SVB-level depegs",
    kPercent: 0.005,
    color: "text-success",
    activeColor: "bg-success/15 ring-1 ring-success/40 text-success",
  },
  medium: {
    label: "Optimal",
    description: "Best for stablecoins",
    kPercent: 0.002,
    color: "text-accent-blue",
    activeColor: "bg-accent-blue/15 ring-1 ring-accent-blue/40 text-accent-blue",
  },
  high: {
    label: "Max",
    description: "Maximum efficiency",
    kPercent: 0.001,
    color: "text-warning",
    activeColor: "bg-warning/15 ring-1 ring-warning/40 text-warning",
  },
};

/** Format a number with 2–4 decimal places. */
function fmt(n: number): string {
  return n.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 4,
  });
}

/** Format depeg price — show "< $0.01" for very small values. */
function fmtDepeg(n: number): string {
  if (n <= 0.005) return "< $0.01";
  return `$${fmt(n)}`;
}

/** Convert a floating k value to Q64.64 raw bigint. */
function kToQ6464Raw(k: number): bigint {
  const SCALE = 1n << 64n;
  const negative = k < 0;
  const abs = Math.abs(k);
  const intPart = BigInt(Math.floor(abs));
  const fracPart = abs - Number(intPart);
  const fracScaled = BigInt(Math.round(fracPart * Number(SCALE)));
  let raw = (intPart << 64n) + fracScaled;
  if (negative) raw = -raw;
  return raw;
}

/**
 * Find an existing tick whose k value is close enough to the target.
 * Tolerance: 0.1% relative difference.
 */
function findMatchingTick(
  ticks: TickInfo[],
  targetK: number,
): TickInfo | undefined {
  return ticks.find((t) => {
    if (t.status !== "Interior") return false;
    const diff = Math.abs(t.kDisplay - targetK) / targetK;
    return diff < 0.001;
  });
}

export function TickSelector({
  pool,
  ticks,
  ticksLoading,
  selection,
  onChange,
}: TickSelectorProps) {
  const [activePreset, setActivePreset] = useState<PresetLevel | null>(null);
  const [kInput, setKInput] = useState("");

  const radius = q6464ToNumber(pool.radius.raw);
  const n = pool.nAssets;
  const kMin = computeKMin(radius, n);
  const kMax = computeKMax(radius, n);

  // Compute k value for a given preset level
  function presetToK(percent: number): number {
    return kMin + (kMax - kMin) * percent;
  }

  // Get current k value (from preset or custom input)
  const currentK = useMemo(() => {
    if (activePreset && activePreset !== "custom") {
      return presetToK(PRESETS[activePreset].kPercent);
    }
    const k = parseFloat(kInput);
    if (!kInput || isNaN(k)) return null;
    return k;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activePreset, kInput, kMin, kMax]);

  // Compute preview for current k
  const kPreview = useMemo(() => {
    if (currentK === null) return null;
    return computeTickPreview(currentK, radius, n);
  }, [currentK, radius, n]);

  // Handle preset selection
  function handlePresetSelect(level: Exclude<PresetLevel, "custom">) {
    setActivePreset(level);
    const k = presetToK(PRESETS[level].kPercent);
    setKInput(k.toFixed(4));

    // Check if an existing tick matches
    const match = findMatchingTick(ticks, k);
    if (match) {
      onChange({ mode: "concentrated", tickAddress: match.address });
    } else {
      onChange({ mode: "concentrated", kRaw: kToQ6464Raw(k) });
    }
  }

  // Handle custom input
  function handleCustomInput(v: string) {
    if (!/^[0-9]*\.?[0-9]*$/.test(v)) return;
    setKInput(v);
    setActivePreset("custom");

    const k = parseFloat(v);
    if (!v || isNaN(k)) {
      onChange({ mode: "concentrated", kRaw: undefined });
      return;
    }

    const match = findMatchingTick(ticks, k);
    if (match) {
      onChange({ mode: "concentrated", tickAddress: match.address });
    } else {
      const raw = kToQ6464Raw(k);
      onChange({ mode: "concentrated", kRaw: raw });
    }
  }

  const interiorTicks = ticks.filter((t) => t.status === "Interior");

  return (
    <div className="space-y-3">
      {/* Mode toggle */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={() => {
            setActivePreset(null);
            onChange({ mode: "full-range" });
          }}
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
          {/* k range info */}
          <div className="flex items-center justify-between rounded-lg bg-surface-2 px-3 py-2 text-[10px]">
            <span className="text-text-tertiary">
              k range: <span className="font-mono text-text-secondary">{fmt(kMin)}</span>
              {" — "}
              <span className="font-mono text-text-secondary">{fmt(kMax)}</span>
            </span>
            <span className="text-text-tertiary">
              {interiorTicks.length} tick{interiorTicks.length !== 1 ? "s" : ""} active
            </span>
          </div>

          {/* Concentration presets */}
          <div>
            <div className="mb-1.5 text-[11px] font-medium text-text-secondary">
              Concentration Level
            </div>
            <div className="grid grid-cols-3 gap-2">
              {(Object.entries(PRESETS) as [Exclude<PresetLevel, "custom">, PresetConfig][]).map(
                ([level, config]) => {
                  const isActive =
                    activePreset === level && selection.mode === "concentrated";
                  const previewK = presetToK(config.kPercent);
                  const preview = computeTickPreview(previewK, radius, n);
                  const hasExisting = findMatchingTick(ticks, previewK);

                  return (
                    <button
                      key={level}
                      type="button"
                      onClick={() => handlePresetSelect(level)}
                      className={`cursor-pointer rounded-lg p-2.5 text-left transition-all ${
                        isActive
                          ? config.activeColor
                          : "bg-surface-2 hover:bg-surface-3 text-text-secondary"
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <span className="text-xs font-semibold">
                          {config.label}
                        </span>
                        {hasExisting && (
                          <Badge variant="success" className="text-[8px]">
                            exists
                          </Badge>
                        )}
                      </div>
                      <div className="mt-0.5 text-[9px] opacity-70">
                        {config.description}
                      </div>
                      {preview && (
                        <div className="mt-1.5 space-y-0.5 text-[9px] opacity-80">
                          <div className="flex justify-between">
                            <span>Concentration</span>
                            <span className="font-mono font-semibold">
                              {fmt(preview.capitalEfficiency)}×
                            </span>
                          </div>
                          <div className="flex justify-between">
                            <span>Depeg</span>
                            <span className="font-mono">
                              {fmtDepeg(preview.depegPrice)}
                            </span>
                          </div>
                        </div>
                      )}
                    </button>
                  );
                },
              )}
            </div>
          </div>

          {/* Custom divider */}
          <div className="flex items-center gap-2">
            <div className="h-px flex-1 bg-border-default" />
            <span className="text-[10px] text-text-tertiary">or custom</span>
            <div className="h-px flex-1 bg-border-default" />
          </div>

          {/* Custom k input */}
          <div className="rounded-lg bg-surface-2 p-3">
            <div className="flex items-center gap-2">
              <label className="text-xs text-text-secondary whitespace-nowrap">
                k =
              </label>
              <input
                type="text"
                inputMode="decimal"
                placeholder={`${fmt(kMin)} – ${fmt(kMax)}`}
                value={kInput}
                onFocus={() => setActivePreset("custom")}
                onChange={(e) => handleCustomInput(e.target.value)}
                className="min-w-0 flex-1 bg-transparent font-mono text-sm text-text-primary outline-none placeholder:text-text-tertiary/40"
              />
            </div>

            {/* k position bar */}
            {kPreview && (
              <div className="mt-2.5">
                <div className="mb-0.5 flex justify-between text-[9px] text-text-tertiary">
                  <span>k_min ({fmt(kPreview.kMin)})</span>
                  <span>k_max ({fmt(kPreview.kMax)})</span>
                </div>
                <div className="relative h-2 rounded-full bg-surface-3">
                  {/* Preset markers */}
                  {Object.values(PRESETS).map((p, i) => (
                    <div
                      key={i}
                      className="absolute top-0 h-2 w-0.5 bg-text-tertiary/30"
                      style={{ left: `${p.kPercent * 100}%` }}
                    />
                  ))}
                  <div
                    className="h-2 rounded-full bg-accent-blue transition-all"
                    style={{
                      width: `${Math.max(0, Math.min(100, kPreview.kPercent))}%`,
                    }}
                  />
                </div>
                <div className="mt-0.5 flex justify-between text-[8px] text-text-tertiary/50">
                  <span>Narrow (concentrated)</span>
                  <span>Wide (full range)</span>
                </div>
              </div>
            )}

            {/* Invalid k warning */}
            {kInput && !kPreview && activePreset === "custom" && (
              <p className="mt-2 text-[10px] text-error">
                k must be between {fmt(kMin)} and {fmt(kMax)}
              </p>
            )}
          </div>

          {/* Tick preview details */}
          {kPreview && (
            <div className="rounded-lg border border-border-subtle bg-surface-1/50 p-3">
              <div className="mb-2 flex items-center justify-between">
                <span className="text-[11px] font-medium text-text-secondary">
                  Tick Preview
                </span>
                <span className="font-mono text-[10px] text-text-tertiary">
                  k = {fmt(kPreview.k)}
                </span>
              </div>
              <div className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[10px]">
                <div className="flex justify-between">
                  <span className="text-text-tertiary">Reserve range</span>
                  <span className="font-mono text-text-secondary">
                    {fmt(kPreview.xMin)} – {fmt(kPreview.xMax)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-text-tertiary">Concentration</span>
                  <span className="font-mono text-accent-blue font-semibold">
                    {fmt(kPreview.capitalEfficiency)}×
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-text-tertiary">Depeg trigger</span>
                  <span className="font-mono text-text-secondary">
                    {fmtDepeg(kPreview.depegPrice)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-text-tertiary">Sphere radius</span>
                  <span className="font-mono text-text-secondary">
                    {fmt(kPreview.boundarySphereRadius)}
                  </span>
                </div>
              </div>
            </div>
          )}

          {/* Info banner */}
          <div className="rounded-lg bg-accent-blue/8 px-3 py-2 text-[10px] text-accent-blue/80">
            Concentration shows how narrowly your liquidity is focused. Higher
            concentration = more fee earnings near peg, but earns nothing when
            the pool trades outside your range. Note: concentration does not
            amplify trading depth in the current version.
          </div>
        </div>
      )}
    </div>
  );
}
