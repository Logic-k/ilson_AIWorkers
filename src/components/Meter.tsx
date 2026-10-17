import type { ReactNode } from "react";

export interface MeterProps {
  value: number;
  max: number;
  label?: ReactNode;
  subLabel?: ReactNode;
  formatValue?: (val: number) => string;
  formatMax?: (max: number) => string;
  warnAt?: number; // ratio (default 0.8)
  badAt?: number; // ratio (default 1.0)
  height?: number;
}

export function Meter({
  value,
  max,
  label,
  subLabel,
  formatValue,
  formatMax,
  warnAt = 0.8,
  badAt = 1.0,
  height = 8,
}: MeterProps) {
  const safeMax = max > 0 ? max : 1;
  const ratio = Math.max(0, value / safeMax);
  const pct = Math.min(100, Math.round(ratio * 100));

  let color = "var(--ok)";
  let toneClass = "meter__fill--ok";
  if (ratio >= badAt) {
    color = "var(--bad)";
    toneClass = "meter__fill--bad";
  } else if (ratio >= warnAt) {
    color = "var(--warn)";
    toneClass = "meter__fill--warn";
  }

  const valStr = formatValue ? formatValue(value) : value.toLocaleString();
  const maxStr = formatMax ? formatMax(max) : max.toLocaleString();

  return (
    <div className="meter" role="meter" aria-valuenow={value} aria-valuemin={0} aria-valuemax={max}>
      {(label || subLabel || max > 0) && (
        <div className="meter__meta">
          <span>{label}</span>
          <span className="meter__val">
            {valStr} / {maxStr} ({pct}%)
          </span>
        </div>
      )}
      <div className="meter__bar" style={{ height }}>
        <svg
          width="100%"
          height={height}
          style={{ display: "block", borderRadius: height / 2, overflow: "hidden" }}
        >
          <rect width="100%" height={height} fill="#eae7df" rx={height / 2} />
          <rect
            width={`${pct}%`}
            height={height}
            fill={color}
            rx={height / 2}
            className={`meter__fill ${toneClass}`}
          />
        </svg>
      </div>
      {subLabel && <div className="meter__meta"><small>{subLabel}</small></div>}
    </div>
  );
}
