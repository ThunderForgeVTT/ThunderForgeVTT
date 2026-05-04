import { Button } from "@/components/ui/button/Button";
import type { DiskUsageBreakdown } from "@/types/admin";
import styles from "./DiskUsageChart.module.scss";

interface DiskUsageChartProps {
  usage: DiskUsageBreakdown;
  onRefresh: () => Promise<void>;
  isRefreshing: boolean;
}

function formatBytes(value: number) {
  if (value <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = value;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size.toFixed(size >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

export function DiskUsageChart({
  usage,
  onRefresh,
  isRefreshing,
}: DiskUsageChartProps) {
  const segments = [
    { label: "Worlds", value: usage.worldsBytes },
    { label: "Assets", value: usage.assetsBytes },
    { label: "Client", value: usage.clientBytes },
    { label: "Databases", value: usage.databasesBytes },
    { label: "Modules", value: usage.modulesBytes },
  ];

  const total = Math.max(usage.totalBytes, 1);

  return (
    <div className={styles.chart}>
      <div className={styles.header}>
        <div>
          <h3>Data footprint</h3>
          <p>Total persisted storage: {formatBytes(usage.totalBytes)}</p>
        </div>
        <Button
          type="button"
          variant="secondary"
          icon="spark"
          onClick={() => void onRefresh()}
          disabled={isRefreshing}
        >
          {isRefreshing ? "Recounting..." : "Recalculate"}
        </Button>
      </div>

      <div className={styles.stack}>
        {segments.map((segment) => (
          <div key={segment.label} className={styles.segment}>
            <div className={styles.segmentHeader}>
              <span>{segment.label}</span>
              <strong>{formatBytes(segment.value)}</strong>
            </div>
            <div className={styles.track} aria-hidden="true">
              <div
                className={styles.fill}
                style={{ width: `${Math.max((segment.value / total) * 100, 3)}%` }}
              />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
