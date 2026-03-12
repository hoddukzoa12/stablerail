"use client";

import React from "react";

type ButtonVariant = "primary" | "secondary" | "ghost" | "gradient";
type ButtonSize = "sm" | "md" | "lg";

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
}

const variantStyles: Record<ButtonVariant, string> = {
  primary:
    "bg-brand-primary text-white hover:bg-brand-primary-hover active:bg-brand-primary-pressed",
  secondary:
    "bg-surface-2 border border-border-default text-text-primary hover:bg-surface-3",
  ghost:
    "bg-transparent text-text-secondary hover:bg-surface-2 hover:text-text-primary",
  gradient:
    "bg-gradient-to-br from-brand-primary to-brand-secondary text-white hover:opacity-90 active:opacity-80",
};

const sizeStyles: Record<ButtonSize, string> = {
  sm: "h-8 px-3 text-xs rounded-md",
  md: "h-10 px-4 text-sm rounded-lg",
  lg: "h-12 px-6 text-base rounded-lg",
};

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "primary", size = "md", className, disabled, children, ...props }, ref) => {
    const base =
      "inline-flex items-center justify-center font-medium transition-colors duration-200 ease-[var(--easing-default)] cursor-pointer select-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand-primary";
    const disabledStyle = disabled ? "opacity-40 cursor-not-allowed pointer-events-none" : "";

    return (
      <button
        ref={ref}
        disabled={disabled}
        className={`${base} ${variantStyles[variant]} ${sizeStyles[size]} ${disabledStyle} ${className ?? ""}`}
        {...props}
      >
        {children}
      </button>
    );
  }
);

Button.displayName = "Button";

export { Button };
export type { ButtonProps, ButtonVariant, ButtonSize };
