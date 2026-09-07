import type { HostRateLimits, RateLimitBucket, RateLimitWindow } from "../types";

interface UsageLimitsProps {
  hosts: HostRateLimits[];
}

interface DisplayWindow {
  key: string;
  label: string;
  hostLabel: string;
  bucketLabel: string;
  remaining: number;
  resetsAt: number | null;
}

export function rateWindowLabel(minutes: number | null): string {
  if (minutes === null) return "Limit";
  if (minutes === 10_080) return "Week";
  if (minutes === 43_200 || minutes === 43_800 || minutes === 44_640) return "Month";
  if (minutes % 10_080 === 0) return `${minutes / 10_080}w`;
  if (minutes % 1_440 === 0) return `${minutes / 1_440}d`;
  if (minutes % 60 === 0) return `${minutes / 60}h`;
  return `${minutes}m`;
}

function windowsFor(host: HostRateLimits, bucket: RateLimitBucket): DisplayWindow[] {
  return (["primary", "secondary"] as const).flatMap((kind) => {
    const window: RateLimitWindow | null = bucket[kind];
    if (!window) return [];
    return [{
      key: `${host.hostId}:${bucket.limitId}:${kind}`,
      label: rateWindowLabel(window.windowDurationMins),
      hostLabel: host.hostLabel,
      bucketLabel: bucket.limitName ?? bucket.limitId,
      remaining: Math.max(0, Math.min(100, Math.round(100 - window.usedPercent))),
      resetsAt: window.resetsAt,
    }];
  });
}

function resetLabel(timestamp: number | null): string {
  if (timestamp === null) return "Reset time unavailable";
  return `Resets ${new Date(timestamp * 1000).toLocaleString()}`;
}

export function UsageLimits({ hosts }: UsageLimitsProps) {
  const windows = hosts.flatMap((host) => host.limits.flatMap((bucket) => windowsFor(host, bucket)));
  const showHost = hosts.filter((host) => host.limits.length > 0).length > 1;
  const showBucket = hosts.some((host) => host.limits.length > 1);

  return (
    <div className="usage-limits" aria-label="Codex usage limits">
      <span className="usage-limits__heading">Codex limits</span>
      {windows.length === 0 ? (
        <span className="usage-limits__unavailable">Unavailable</span>
      ) : windows.map((window) => {
        const detail = [showHost ? window.hostLabel : null, showBucket ? window.bucketLabel : null, window.label]
          .filter(Boolean)
          .join(" · ");
        const severity = window.remaining <= 10 ? "critical" : window.remaining <= 25 ? "low" : "normal";
        return (
          <span
            className={`usage-limit usage-limit--${severity}`}
            key={window.key}
            title={`${window.hostLabel} · ${window.bucketLabel} · ${resetLabel(window.resetsAt)}`}
            aria-label={`${detail}: ${window.remaining}% left. ${resetLabel(window.resetsAt)}`}
          >
            <small>{detail}</small><strong>{window.remaining}%</strong><em>left</em>
          </span>
        );
      })}
    </div>
  );
}
