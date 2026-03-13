import { Card } from "../ui/card";
import { Badge } from "../ui/badge";
import { formatUsd, truncateAddress, explorerUrl } from "../../lib/format-utils";
import type { PolicyStateData } from "../../lib/settlement-deserializer";

interface PolicyStatusCardProps {
  policy: PolicyStateData;
}

function getUsageBarColor(percent: number): string {
  if (percent > 80) return "var(--error)";
  if (percent > 50) return "var(--warning)";
  return "var(--success)";
}

function MetricRow({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-sm text-text-secondary">{label}</span>
      {children}
    </div>
  );
}

export function PolicyStatusCard({ policy }: PolicyStatusCardProps) {
  const dailyUsagePercent =
    policy.maxDailyVolume > 0
      ? (policy.currentDailyVolume / policy.maxDailyVolume) * 100
      : 0;

  const lastReset = policy.lastResetTimestamp
    ? new Date(policy.lastResetTimestamp * 1000).toLocaleString()
    : "Never";

  return (
    <Card variant="glass">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium uppercase tracking-wider text-text-tertiary">
          Policy Status
        </h3>
        <Badge variant={policy.isActive ? "success" : "error"}>
          {policy.isActive ? "Active" : "Inactive"}
        </Badge>
      </div>

      <div className="mt-4 space-y-3">
        <MetricRow label="Max Trade Amount">
          <span className="font-mono text-sm text-text-primary">
            {formatUsd(policy.maxTradeAmount)}
          </span>
        </MetricRow>

        <MetricRow label="Max Daily Volume">
          <span className="font-mono text-sm text-text-primary">
            {formatUsd(policy.maxDailyVolume)}
          </span>
        </MetricRow>

        <div>
          <MetricRow label="Daily Volume Used">
            <span className="font-mono text-sm text-text-primary">
              {formatUsd(policy.currentDailyVolume)}{" "}
              <span className="text-text-tertiary">
                ({dailyUsagePercent.toFixed(1)}%)
              </span>
            </span>
          </MetricRow>
          <div className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-surface-3">
            <div
              className="h-full rounded-full transition-all duration-300"
              style={{
                width: `${Math.min(dailyUsagePercent, 100)}%`,
                backgroundColor: getUsageBarColor(dailyUsagePercent),
              }}
            />
          </div>
        </div>

        <div className="border-t border-border-subtle" />

        <MetricRow label="Authority">
          <a
            href={explorerUrl("address", policy.authority)}
            target="_blank"
            rel="noopener noreferrer"
            className="font-mono text-sm text-brand-primary hover:underline"
          >
            {truncateAddress(policy.authority)}
          </a>
        </MetricRow>

        <MetricRow label="Last Reset">
          <span className="text-sm text-text-secondary">{lastReset}</span>
        </MetricRow>
      </div>
    </Card>
  );
}
