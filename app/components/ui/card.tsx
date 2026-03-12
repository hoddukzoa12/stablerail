import React from "react";

type CardVariant = "default" | "glass" | "gradient-border";

interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  variant?: CardVariant;
}

const variantStyles: Record<CardVariant, string> = {
  default: "bg-surface-1 border border-border-default rounded-xl p-6",
  glass: "glass-panel rounded-xl p-6",
  "gradient-border": "gradient-border rounded-xl p-6",
};

function Card({ variant = "default", className, children, ...props }: CardProps) {
  return (
    <div className={`${variantStyles[variant]} ${className ?? ""}`} {...props}>
      {children}
    </div>
  );
}

export { Card };
export type { CardProps, CardVariant };
