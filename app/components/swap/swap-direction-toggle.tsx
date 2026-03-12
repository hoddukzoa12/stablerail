"use client";

import { ArrowDownUp } from "lucide-react";

interface SwapDirectionToggleProps {
  onToggle: () => void;
}

export function SwapDirectionToggle({ onToggle }: SwapDirectionToggleProps) {
  return (
    <div className="relative z-10 -my-2 flex justify-center">
      <button
        type="button"
        onClick={onToggle}
        className="flex h-9 w-9 items-center justify-center rounded-full border-4 border-surface-1 bg-surface-3 text-text-secondary transition-all hover:rotate-180 hover:bg-brand-primary hover:text-white cursor-pointer"
        style={{ transitionDuration: "300ms" }}
        aria-label="Switch swap direction"
      >
        <ArrowDownUp className="h-4 w-4" />
      </button>
    </div>
  );
}
