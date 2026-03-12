"use client";

import { useState, useRef, useEffect } from "react";
import Image from "next/image";
import { ChevronDown } from "lucide-react";
import { TOKENS, type TokenInfo } from "../../lib/tokens";

interface TokenSelectorProps {
  selected: TokenInfo;
  onSelect: (token: TokenInfo) => void;
  /** Token index to disable (the other side of the swap) */
  disabledIndex?: number;
}

export function TokenSelector({
  selected,
  onSelect,
  disabledIndex,
}: TokenSelectorProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

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

  return (
    <div className="relative" ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center gap-2 rounded-full bg-surface-2 px-3 py-1.5 text-sm font-medium text-text-primary transition-colors hover:bg-surface-3 cursor-pointer"
      >
        <Image
          src={selected.icon}
          alt={selected.symbol}
          width={20}
          height={20}
          className="rounded-full"
        />
        <span>{selected.symbol}</span>
        <ChevronDown className="h-3.5 w-3.5 text-text-tertiary" />
      </button>

      {open && (
        <div className="absolute left-0 top-full z-50 mt-2 w-44 rounded-xl border border-border-default bg-surface-1 py-1 shadow-lg">
          {TOKENS.map((token) => {
            const isDisabled = token.index === disabledIndex;
            const isSelected = token.index === selected.index;

            return (
              <button
                key={token.symbol}
                type="button"
                disabled={isDisabled}
                onClick={() => {
                  if (!isDisabled) {
                    onSelect(token);
                    setOpen(false);
                  }
                }}
                className={`flex w-full items-center gap-3 px-3 py-2.5 text-sm transition-colors cursor-pointer ${
                  isDisabled
                    ? "cursor-not-allowed opacity-30"
                    : isSelected
                      ? "bg-surface-2 text-text-primary"
                      : "text-text-secondary hover:bg-surface-2 hover:text-text-primary"
                }`}
              >
                <Image
                  src={token.icon}
                  alt={token.symbol}
                  width={24}
                  height={24}
                  className="rounded-full"
                />
                <div className="flex flex-col items-start">
                  <span className="font-medium">{token.symbol}</span>
                  <span className="text-[11px] text-text-tertiary">
                    {token.name}
                  </span>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
