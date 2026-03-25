'use client';

import { useApi } from '@/hooks/useApi';

interface HealthData {
  status: string;
  last_slot?: number;
}

export default function WelcomeBanner() {
  const { data } = useApi<HealthData>('/health', { interval: 5000 });
  const isOk = data?.status === 'ok';

  return (
    <div
      style={{
        background: 'linear-gradient(135deg, var(--bg-card) 0%, var(--bg-secondary) 100%)',
        border: '1px solid var(--border)',
        borderRadius: 8,
        padding: '28px 32px',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        flexWrap: 'wrap',
        gap: 20,
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      {/* Accent line at top */}
      <div
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          height: 1,
          background: 'linear-gradient(90deg, transparent 0%, var(--accent) 30%, var(--green) 70%, transparent 100%)',
          opacity: 0.4,
        }}
      />

      <div>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, marginBottom: 6 }}>
          <h2
            style={{
              fontFamily: 'var(--mono)',
              fontSize: 28,
              fontWeight: 700,
              letterSpacing: '-1px',
              lineHeight: 1,
              background: 'linear-gradient(135deg, var(--accent) 0%, var(--green) 100%)',
              WebkitBackgroundClip: 'text',
              WebkitTextFillColor: 'transparent',
              backgroundClip: 'text',
            }}
          >
            SolTrace
          </h2>
          <span
            style={{
              fontFamily: 'var(--mono)',
              fontSize: 9,
              color: 'var(--text-muted)',
              letterSpacing: '2px',
              textTransform: 'uppercase',
              border: '1px solid var(--border)',
              padding: '2px 8px',
              borderRadius: 3,
            }}
          >
            Indexer
          </span>
        </div>
        <p style={{ fontSize: 13, color: 'var(--text-secondary)', lineHeight: 1.6, maxWidth: 520 }}>
          Real-time Solana blockchain indexer tracking deposits, withdrawals, and token transfers.
        </p>
      </div>

      <div className="banner-status" style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        {/* Status indicator */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            background: 'var(--bg-surface)',
            border: '1px solid var(--border)',
            borderRadius: 4,
            padding: '7px 14px',
          }}
        >
          <span className={`status-dot ${isOk ? 'online' : 'connecting'}`} />
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, fontWeight: 500, color: isOk ? 'var(--green)' : 'var(--yellow)', letterSpacing: '0.5px', textTransform: 'uppercase' }}>
            {isOk ? 'Online' : 'Connecting'}
          </span>
        </div>

        {/* Slot counter */}
        {data?.last_slot && (
          <div
            style={{
              background: 'var(--bg-surface)',
              border: '1px solid var(--border)',
              borderRadius: 4,
              padding: '7px 14px',
              fontFamily: 'var(--mono)',
              fontSize: 11,
              color: 'var(--text-muted)',
              letterSpacing: '0.5px',
            }}
          >
            <span style={{ color: 'var(--text-secondary)', marginRight: 6 }}>SLOT</span>
            <span style={{ color: 'var(--accent)', fontWeight: 600 }}>{data.last_slot.toLocaleString()}</span>
          </div>
        )}
      </div>
    </div>
  );
}
