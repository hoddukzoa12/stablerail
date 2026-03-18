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

import { useState, useMemo, useEffect, useCallback, useRef } from "react";
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

/**
 * Convert a k value to Q64.64 raw bigint.
 *
 * Accepts either a number or a string. When a string is provided (e.g. from
 * user input), the decimal is decomposed directly in string space to avoid
 * IEEE-754 rounding artifacts (e.g. `parseFloat("0.3")` → 0.29999...99944).
 * This is critical for `findMatchingTick` which uses exact bigint equality —
 * a 1-ULP difference in kRaw produces a different PDA.
 *
 * When a number is provided (e.g. from preset computations), falls back to
 * `toFixed(18)` string conversion — acceptable for presets since those values
 * are computed from exact integer math on the sphere radius.
 */
function kToQ6464Raw(k: number | string): bigint {
  const SCALE = 1n << 64n;

  // Normalize to string, preserving user input precision when possible
  const str = typeof k === "string" ? k : Math.abs(k).toFixed(18);
  const negative = typeof k === "number" ? k < 0 : str.startsWith("-");
  const absStr = negative && typeof k === "string" ? str.slice(1) : str;

  // Decompose decimal string into integer + fractional parts.
  // Note: "3.".split(".") → ["3", ""] — the default "0" only applies when
  // the array has no second element (no dot), not when it's an empty string.
  const [intStr, rawFrac = ""] = absStr.split(".");
  const fracStr = rawFrac || "0";
  const intPart = BigInt(intStr || "0");

  // fracScaled = fracStr * 2^64 / 10^fracLen — all in BigInt
  const fracLen = fracStr.length;
  const fracNumerator = BigInt(fracStr) * SCALE;
  const fracDenominator = 10n ** BigInt(fracLen);
  const fracScaled = fracNumerator / fracDenominator;

  let raw = (intPart << 64n) + fracScaled;
  if (negative) raw = -raw;
  return raw;
}

/**
 * Find an existing tick whose k_raw matches the target exactly.
 *
 * Compares Q64.64 bigint values directly rather than float approximation
 * to ensure PDA derivation consistency — even a 1-ULP difference in kRaw
 * produces a different PDA, causing add_liquidity to target a nonexistent account.
 */
function findMatchingTick(
  ticks: TickInfo[],
  targetKRaw: bigint,
): TickInfo | undefined {
  return ticks.find((t) => {
    return t.kRaw === targetKRaw;
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

  // Stable ref for onChange to avoid infinite useEffect loops.
  // Parent may not memoize onChange, so using it directly in deps
  // would re-trigger the effect on every render → onChange → re-render → loop.
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  const radius = q6464ToNumber(pool.radius.raw);
  const n = pool.nAssets;
  const kMin = computeKMin(radius, n);
  const kMax = computeKMax(radius, n);

  // Compute k value for a given preset level (stable ref for useEffect deps)
  const presetToK = useCallback(
    (percent: number): number => kMin + (kMax - kMin) * percent,
    [kMin, kMax],
  );

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

  // Resolve a k value to either an existing tick address or a new kRaw,
  // then notify the parent. Extracted to avoid 3x copy-paste of the
  // findMatchingTick → onChange pattern.
  function resolveTickForK(k: number | string) {
    const raw = kToQ6464Raw(k);
    const match = findMatchingTick(ticks, raw);
    if (match) {
      onChangeRef.current({ mode: "concentrated", tickAddress: match.address });
    } else {
      onChangeRef.current({ mode: "concentrated", kRaw: raw });
    }
  }

  // Handle preset selection
  function handlePresetSelect(level: Exclude<PresetLevel, "custom">) {
    setActivePreset(level);
    const k = presetToK(PRESETS[level].kPercent);
    setKInput(k.toFixed(4));
    resolveTickForK(k);
  }

  // Handle custom input — pass string directly to kToQ6464Raw to avoid
  // parseFloat precision loss (e.g. "0.3" → 0.29999...99944).
  function handleCustomInput(v: string) {
    if (!/^[0-9]*\.?[0-9]*$/.test(v)) return;
    setKInput(v);
    setActivePreset("custom");

    if (!v || v === "." || parseFloat(v) === 0) {
      onChangeRef.current({ mode: "concentrated", kRaw: undefined });
      return;
    }

    resolveTickForK(v);
  }

  // Sync preset kRaw with pool radius changes. When the pool refreshes
  // (e.g., after another LP's deposit or a swap), kMin/kMax shift, so the
  // preset's k and kRaw must be recomputed. Without this, the stale kRaw
  // could fall outside the new [k_min, k_max] and fail on-chain.
  //
  // Uses onChangeRef (not onChange directly) to avoid infinite loop:
  // onChange in deps → parent re-render → new onChange ref → effect re-fires.
  useEffect(() => {
    if (
      activePreset &&
      activePreset !== "custom" &&
      selection.mode === "concentrated" &&
      !selection.tickAddress
    ) {
      const k = presetToK(PRESETS[activePreset].kPercent);
      setKInput(k.toFixed(4));
      const raw = kToQ6464Raw(k);
      const match = findMatchingTick(ticks, raw);
      if (match) {
        onChangeRef.current({ mode: "concentrated", tickAddress: match.address });
      } else {
        onChangeRef.current({ mode: "concentrated", kRaw: raw });
      }
    }
  }, [presetToK, activePreset, selection.mode, selection.tickAddress, ticks]);

  // All ticks shown (not just Interior) since add_liquidity accepts
  // both Interior and Boundary ticks with correct accounting.

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
              {ticks.length} tick{ticks.length !== 1 ? "s" : ""} active
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
                  const hasExisting = findMatchingTick(ticks, kToQ6464Raw(previewK));

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

          {/* Existing ticks picker — bypasses exact kRaw match issues */}
          {ticks.length > 0 && (
            <div>
              <div className="mb-1.5 text-[11px] font-medium text-text-secondary">
                Existing Ticks
              </div>
              <div className="max-h-32 space-y-1 overflow-y-auto rounded-lg bg-surface-2 p-2">
                {ticks.map((t) => {
                  const isSelected = selection.tickAddress === t.address;
                  return (
                    <button
                      key={t.address}
                      type="button"
                      onClick={() => {
                        setActivePreset(null);
                        setKInput(t.kDisplay.toFixed(4));
                        onChange({ mode: "concentrated", tickAddress: t.address });
                      }}
                      className={`w-full cursor-pointer rounded-md px-2.5 py-1.5 text-left transition-all ${
                        isSelected
                          ? "bg-accent-blue/20 ring-1 ring-accent-blue/40"
                          : "hover:bg-surface-3"
                      }`}
                    >
                      <div className="flex items-center justify-between text-[10px]">
                        <span className="font-mono text-text-primary">
                          k = {fmt(t.kDisplay)}
                        </span>
                        <div className="flex items-center gap-1.5">
                          <span className="font-mono text-accent-blue font-semibold">
                            {fmt(t.capitalEfficiency)}×
                          </span>
                          <Badge
                            variant={t.status === "Interior" ? "success" : "warning"}
                            className="text-[8px]"
                          >
                            {t.status}
                          </Badge>
                        </div>
                      </div>
                      {t.liquidityDisplay > 0 && (
                        <div className="mt-0.5 text-[9px] text-text-tertiary">
                          Liquidity: {fmt(t.liquidityDisplay)} · Depeg: {fmtDepeg(t.depegPrice)}
                        </div>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>
          )}

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
