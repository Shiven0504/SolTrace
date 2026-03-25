'use client';

import { useMemo, useState, useCallback } from 'react';
import {
  ComposedChart,
  Line,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
  ReferenceLine,
} from 'recharts';
import { useApi } from '@/hooks/useApi';

interface Transfer {
  block_time: string | null;
  direction: string;
  amount: number;
}

interface ChartPoint {
  time: string;
  total: number;
  deposits: number;
  withdrawals: number;
  depAmount: number;
  wdAmount: number;
  cumulative: number;
}

type TimeRange = '1h' | '6h' | '24h' | '7d' | 'all';

const RANGES: { key: TimeRange; label: string; hours: number }[] = [
  { key: '1h', label: '1H', hours: 1 },
  { key: '6h', label: '6H', hours: 6 },
  { key: '24h', label: '24H', hours: 24 },
  { key: '7d', label: '7D', hours: 168 },
  { key: 'all', label: 'ALL', hours: Infinity },
];

export default function TransferChart() {
  const { data: transfers } = useApi<Transfer[]>('/transfers?limit=1000', { interval: 10000 });
  const [range, setRange] = useState<TimeRange>('all');
  const [hoveredPoint, setHoveredPoint] = useState<ChartPoint | null>(null);

  const chartData = useMemo<ChartPoint[]>(() => {
    if (!transfers || transfers.length === 0) return [];

    const now = Date.now();
    const rangeHours = RANGES.find((r) => r.key === range)!.hours;
    const cutoff = rangeHours === Infinity ? 0 : now - rangeHours * 3600_000;

    const bucketMs = rangeHours <= 1 ? 5 * 60_000
      : rangeHours <= 6 ? 15 * 60_000
      : rangeHours <= 24 ? 60 * 60_000
      : rangeHours <= 168 ? 6 * 60 * 60_000
      : 60 * 60_000;

    const groups: Record<string, ChartPoint> = {};

    for (const t of transfers) {
      if (!t.block_time) continue;
      const ts = new Date(t.block_time).getTime();
      if (ts < cutoff) continue;

      const bucket = new Date(Math.floor(ts / bucketMs) * bucketMs).toISOString();

      if (!groups[bucket]) {
        groups[bucket] = { time: bucket, total: 0, deposits: 0, withdrawals: 0, depAmount: 0, wdAmount: 0, cumulative: 0 };
      }
      groups[bucket].total += 1;
      if (t.direction === 'deposit') {
        groups[bucket].deposits += 1;
        groups[bucket].depAmount += t.amount;
      } else {
        groups[bucket].withdrawals += 1;
        groups[bucket].wdAmount += t.amount;
      }
    }

    const sorted = Object.values(groups).sort((a, b) => a.time.localeCompare(b.time));

    let cum = 0;
    for (const p of sorted) {
      cum += p.total;
      p.cumulative = cum;
    }

    return sorted;
  }, [transfers, range]);

  const stats = useMemo(() => {
    if (!transfers || transfers.length === 0) return { total: 0, deps: 0, wds: 0, net: 0 };
    const now = Date.now();
    const rangeHours = RANGES.find((r) => r.key === range)!.hours;
    const cutoff = rangeHours === Infinity ? 0 : now - rangeHours * 3600_000;

    let deps = 0, wds = 0, depAmt = 0, wdAmt = 0;
    for (const t of transfers) {
      if (!t.block_time) continue;
      if (new Date(t.block_time).getTime() < cutoff) continue;
      if (t.direction === 'deposit') { deps++; depAmt += t.amount; }
      else { wds++; wdAmt += t.amount; }
    }
    return { total: deps + wds, deps, wds, net: (depAmt - wdAmt) / 1_000_000_000 };
  }, [transfers, range]);

  const formatTime = useCallback((str: string) => {
    try {
      const d = new Date(str);
      const rangeHours = RANGES.find((r) => r.key === range)!.hours;
      if (rangeHours <= 24) {
        return d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
      }
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
    } catch {
      return str;
    }
  }, [range]);

  const formatFullTime = (str: string) => {
    try {
      const d = new Date(str);
      return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' }) +
        ' ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    } catch {
      return str;
    }
  };

  const displayPoint = hoveredPoint || (chartData.length > 0 ? chartData[chartData.length - 1] : null);
  const hasAnyData = transfers && transfers.length > 0;
  const hasRangeData = chartData.length > 0;

  if (!hasAnyData) {
    return (
      <div className="card">
        <h3>Transfer Activity</h3>
        <div className="empty-state">No transfer data to chart yet</div>
      </div>
    );
  }

  return (
    <div className="card" style={{ padding: 0 }}>
      {/* Header */}
      <div className="chart-header" style={{ padding: '18px 22px 0', display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', flexWrap: 'wrap', gap: 12 }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 6 }}>
            <h3 style={{ marginBottom: 0 }}>Transfer Activity</h3>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '0.5px' }}>
              {displayPoint ? formatFullTime(displayPoint.time) : ''}
            </span>
          </div>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 20 }}>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 28, fontWeight: 700, letterSpacing: '-1px', color: 'var(--text-primary)' }}>
              {displayPoint?.total ?? 0}
              <span style={{ fontSize: 11, fontWeight: 400, color: 'var(--text-muted)', marginLeft: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>txns</span>
            </span>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--green)', fontWeight: 600, letterSpacing: '0.5px' }}>
              {displayPoint?.deposits ?? 0} in
            </span>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--red)', fontWeight: 600, letterSpacing: '0.5px' }}>
              {displayPoint?.withdrawals ?? 0} out
            </span>
          </div>
          {/* Period stats */}
          <div style={{ display: 'flex', gap: 16, marginTop: 8 }}>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '0.5px' }}>
              PERIOD <strong style={{ color: 'var(--text-secondary)' }}>{stats.total}</strong> txns
            </span>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '0.5px' }}>
              NET <strong style={{ color: stats.net >= 0 ? 'var(--green)' : 'var(--red)', textShadow: stats.net >= 0 ? '0 0 8px var(--green-glow)' : '0 0 8px var(--red-glow)' }}>
                {stats.net >= 0 ? '+' : ''}{stats.net.toFixed(4)} SOL
              </strong>
            </span>
          </div>
        </div>

        {/* Time range selector */}
        <div style={{ display: 'flex', gap: 1, background: 'var(--bg-surface)', borderRadius: 4, padding: 2, border: '1px solid var(--border)' }}>
          {RANGES.map((r) => (
            <button
              key={r.key}
              onClick={() => setRange(r.key)}
              style={{
                padding: '4px 12px',
                borderRadius: 3,
                border: 'none',
                fontFamily: 'var(--mono)',
                fontSize: 10,
                fontWeight: range === r.key ? 600 : 400,
                letterSpacing: '1px',
                cursor: 'pointer',
                background: range === r.key ? 'var(--accent)' : 'transparent',
                color: range === r.key ? 'var(--bg-primary)' : 'var(--text-muted)',
                transition: 'all 0.15s',
              }}
            >
              {r.label}
            </button>
          ))}
        </div>
      </div>

      {hasRangeData ? (
        <>
          {/* Cumulative line chart */}
          <div style={{ padding: '12px 8px 0' }}>
            <ResponsiveContainer width="100%" height={220}>
              <ComposedChart
                data={chartData}
                margin={{ top: 8, right: 8, left: -16, bottom: 0 }}
                onMouseMove={(state: { activePayload?: Array<{ payload: ChartPoint }> }) => {
                  if (state?.activePayload?.[0]) {
                    setHoveredPoint(state.activePayload[0].payload);
                  }
                }}
                onMouseLeave={() => setHoveredPoint(null)}
              >
                <defs>
                  <linearGradient id="lineGrad" x1="0" y1="0" x2="1" y2="0">
                    <stop offset="0%" stopColor="#00e5ff" />
                    <stop offset="100%" stopColor="#22c55e" />
                  </linearGradient>
                  <linearGradient id="areaFill" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="0%" stopColor="#00e5ff" stopOpacity={0.12} />
                    <stop offset="100%" stopColor="#00e5ff" stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border-subtle)" strokeOpacity={0.6} vertical={false} />
                <XAxis
                  dataKey="time"
                  tickFormatter={formatTime}
                  tick={{ fontSize: 9, fill: 'var(--text-muted)', fontFamily: 'IBM Plex Mono, monospace' }}
                  axisLine={false}
                  tickLine={false}
                  minTickGap={40}
                />
                <YAxis
                  yAxisId="line"
                  allowDecimals={false}
                  tick={{ fontSize: 9, fill: 'var(--text-muted)', fontFamily: 'IBM Plex Mono, monospace' }}
                  axisLine={false}
                  tickLine={false}
                  width={40}
                />
                <Tooltip content={() => null} />
                {hoveredPoint && (
                  <ReferenceLine
                    yAxisId="line"
                    x={hoveredPoint.time}
                    stroke="var(--accent)"
                    strokeDasharray="2 4"
                    strokeOpacity={0.4}
                  />
                )}
                <Line
                  yAxisId="line"
                  type="monotone"
                  dataKey="cumulative"
                  stroke="url(#lineGrad)"
                  strokeWidth={2}
                  dot={false}
                  activeDot={{ r: 3, fill: '#00e5ff', stroke: 'var(--bg-primary)', strokeWidth: 2 }}
                  fill="url(#areaFill)"
                />
              </ComposedChart>
            </ResponsiveContainer>
          </div>

          {/* Volume bars */}
          <div style={{ padding: '0 8px 14px' }}>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--text-muted)', padding: '0 0 4px 44px', letterSpacing: '2px' }}>VOL</div>
            <ResponsiveContainer width="100%" height={56}>
              <ComposedChart data={chartData} margin={{ top: 0, right: 8, left: -16, bottom: 0 }}>
                <XAxis dataKey="time" hide />
                <YAxis allowDecimals={false} tick={{ fontSize: 9, fill: 'var(--text-muted)', fontFamily: 'IBM Plex Mono, monospace' }} axisLine={false} tickLine={false} width={40} />
                <Tooltip content={() => null} />
                <Bar dataKey="deposits" stackId="vol" fill="var(--green)" fillOpacity={0.6} radius={[1, 1, 0, 0]} />
                <Bar dataKey="withdrawals" stackId="vol" fill="var(--red)" fillOpacity={0.6} radius={[1, 1, 0, 0]} />
              </ComposedChart>
            </ResponsiveContainer>
          </div>
        </>
      ) : (
        <div style={{ padding: '48px 20px', textAlign: 'center', color: 'var(--text-muted)', fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '1px', textTransform: 'uppercase' }}>
          No transfers in selected range
        </div>
      )}

      {/* Legend */}
      <div className="chart-legend" style={{ padding: '0 22px 16px', display: 'flex', gap: 20, fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.5px' }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--text-muted)' }}>
          <span style={{ width: 16, height: 2, background: 'linear-gradient(90deg, #00e5ff, #22c55e)', borderRadius: 1, display: 'inline-block' }} />
          Cumulative
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--text-muted)' }}>
          <span style={{ width: 8, height: 8, background: 'var(--green)', borderRadius: 1, display: 'inline-block', opacity: 0.6 }} />
          Deposits
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--text-muted)' }}>
          <span style={{ width: 8, height: 8, background: 'var(--red)', borderRadius: 1, display: 'inline-block', opacity: 0.6 }} />
          Withdrawals
        </span>
      </div>
    </div>
  );
}
