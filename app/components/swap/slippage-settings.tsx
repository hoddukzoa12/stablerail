"use client";

import { useState, useRef, useEffect } from "react";
import { Settings } from "lucide-react";

interface SlippageSettingsProps {
  slippageBps: number;
  onSlippageChange: (bps: number) => void;
}

const PRESETS = [
  { label: "0.1%", bps: 10 },
  { label: "0.5%", bps: 50 },
  { label: "1.0%", bps: 100 },
];

export function SlippageSettings({
  slippageBps,
  onSlippageChange,
}: SlippageSettingsProps) {
  const [open, setOpen] = useState(false);
  const [customInput, setCustomInput] = useState("");
  const ref = useRef<HTMLDivElement>(null);

  const isCustom = !PRESETS.some((p) => p.bps === slippageBps);

  // Close on outside click
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Persist to localStorage
  useEffect(() => {
    localStorage.setItem("stablerail-slippage-bps", String(slippageBps));
  }, [slippageBps]);

  const handleCustomSubmit = () => {
    const parsed = parseFloat(customInput);
    if (!isNaN(parsed) && parsed > 0 && parsed <= 50) {
      onSlippageChange(Math.round(parsed * 100));
      setCustomInput("");
    }
  };

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center justify-center rounded-lg p-1.5 text-text-tertiary transition-colors hover:bg-surface-2 hover:text-text-secondary cursor-pointer"
        aria-label="Slippage settings"
      >
        <Settings className="h-4.5 w-4.5" />
      </button>

      {open && (
        <div className="absolute right-0 top-full z-50 mt-2 w-64 rounded-xl border border-border-default bg-surface-1 p-4 shadow-lg">
          <p className="mb-3 text-xs font-medium uppercase tracking-wider text-text-tertiary">
            Max Slippage
          </p>

          {/* Preset buttons */}
          <div className="mb-3 flex gap-2">
            {PRESETS.map((preset) => (
              <button
                key={preset.bps}
                type="button"
                onClick={() => {
                  onSlippageChange(preset.bps);
                  setCustomInput("");
                }}
                className={`flex-1 rounded-lg py-2 text-sm font-medium transition-colors cursor-pointer ${
                  slippageBps === preset.bps
                    ? "bg-brand-primary text-white"
                    : "bg-surface-2 text-text-secondary hover:bg-surface-3"
                }`}
              >
                {preset.label}
              </button>
            ))}
          </div>

          {/* Custom input */}
          <div className="flex items-center gap-2">
            <div
              className={`flex flex-1 items-center rounded-lg border px-3 py-2 transition-colors ${
                isCustom
                  ? "border-brand-primary"
                  : "border-border-default"
              }`}
            >
              <input
                type="text"
                inputMode="decimal"
                placeholder="Custom"
                value={
                  isCustom && !customInput
                    ? (slippageBps / 100).toString()
                    : customInput
                }
                onChange={(e) => {
                  if (/^[0-9]*\.?[0-9]*$/.test(e.target.value)) {
                    setCustomInput(e.target.value);
                  }
                }}
                onKeyDown={(e) => {
                  if (e.key === "Enter") handleCustomSubmit();
                }}
                onBlur={handleCustomSubmit}
                className="min-w-0 flex-1 bg-transparent text-sm text-text-primary outline-none placeholder:text-text-tertiary"
              />
              <span className="ml-1 text-sm text-text-tertiary">%</span>
            </div>
          </div>

          {/* Warning for high slippage */}
          {slippageBps > 100 && (
            <p className="mt-2 text-[11px] text-warning">
              High slippage increases the risk of a worse execution price.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
