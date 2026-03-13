"use client";

import { clampPercent } from "../../lib/format-utils";

interface PercentageSelectorProps {
  value: number;
  onChange: (percent: number) => void;
  /** Smaller sizing for inline usage (default: false) */
  compact?: boolean;
}

const PRESETS = [25, 50, 75, 100] as const;

/**
 * Reusable percentage selector with 25/50/75/100 preset buttons
 * and a custom numeric input (1-100).
 */
export function PercentageSelector({
  value,
  onChange,
  compact = false,
}: PercentageSelectorProps) {
  const btnPy = compact ? "py-0.5" : "py-1.5";
  const btnText = compact ? "text-[10px]" : "text-[11px]";
  const inputW = compact ? "w-14" : "w-16";
  const inputPy = compact ? "py-0.5" : "py-1.5";
  const inputPl = compact ? "pl-1.5" : "pl-2";
  const inputPr = compact ? "pr-4" : "pr-5";
  const inputText = compact ? "text-[10px]" : "text-[11px]";
  const pctText = compact ? "text-[9px]" : "text-[10px]";
  const pctRight = compact ? "right-1" : "right-1.5";

  return (
    <div className="flex items-center gap-1.5">
      {PRESETS.map((pct) => (
        <button
          key={pct}
          type="button"
          onClick={() => onChange(pct)}
          className={`flex-1 rounded-md ${btnPy} ${btnText} font-semibold transition-colors ${
            value === pct
              ? "bg-brand-primary/20 text-brand-primary"
              : "bg-surface-3 text-text-tertiary hover:text-text-secondary"
          }`}
        >
          {pct}%
        </button>
      ))}
      <div className={`relative flex ${inputW} shrink-0 items-center`}>
        <input
          type="number"
          min={1}
          max={100}
          value={value}
          onChange={(e) => onChange(clampPercent(e.target.value))}
          className={`w-full rounded-md bg-surface-3 ${inputPy} ${inputPl} ${inputPr} font-mono ${inputText} font-semibold text-text-primary outline-none focus:ring-1 focus:ring-brand-primary/40`}
        />
        <span
          className={`pointer-events-none absolute ${pctRight} ${pctText} text-text-tertiary`}
        >
          %
        </span>
      </div>
    </div>
  );
}
