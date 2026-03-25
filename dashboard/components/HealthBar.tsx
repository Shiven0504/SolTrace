'use client';

import { useApi } from '@/hooks/useApi';

interface HealthData {
  status: string;
  last_slot?: number;
  rpc_url?: string;
}

interface BackfillJob {
  id: number;
  wallet: string;
  status: string;
  total_fetched: number;
  total_indexed: number;
  created_at: string;
}

interface Wallet {
  wallet_pubkey: string;
}

export default function HealthBar() {
  const { data, error } = useApi<HealthData>('/health', { interval: 5000 });
  const { data: wallets } = useApi<Wallet[]>('/wallets', { interval: 10000 });
  const { data: backfillJobs } = useApi<BackfillJob[]>('/backfill', { interval: 10000 });

  const isOk = !error && data?.status === 'ok';
  const statusColor = error ? 'var(--red)' : isOk ? 'var(--green)' : 'var(--yellow)';
  const statusText = error ? 'Offline' : isOk ? 'Online' : 'Connecting';
  const statusClass = error ? 'offline' : isOk ? 'online' : 'connecting';

  const runningJobs = backfillJobs?.filter((j) => j.status === 'running') ?? [];

  return (
    <div className="stagger" style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
      <div>
        <h2 style={{ fontSize: 20, fontWeight: 700, letterSpacing: '-0.3px', marginBottom: 4 }}>Indexer Health</h2>
        <p style={{ fontSize: 13, color: 'var(--text-secondary)' }}>
          Monitor the real-time status of the SolTrace indexer.
        </p>
      </div>

      <div className="stats-grid">
        <div className="card">
          <h3>Status</h3>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span className={`status-dot ${statusClass}`} />
            <span className="stat-value" style={{ fontSize: 22, color: statusColor, textShadow: `0 0 16px ${statusColor === 'var(--green)' ? 'var(--green-glow)' : statusColor === 'var(--red)' ? 'var(--red-glow)' : 'var(--yellow-dim)'}` }}>
              {statusText}
            </span>
          </div>
        </div>
        <div className="card">
          <h3>Last Slot</h3>
          <div className="stat-value" style={{ fontSize: 22, color: 'var(--accent)', textShadow: '0 0 16px var(--accent-glow)' }}>
            {data?.last_slot ? data.last_slot.toLocaleString() : '-'}
          </div>
        </div>
        <div className="card">
          <h3>Watched Wallets</h3>
          <div className="stat-value" style={{ fontSize: 22, color: 'var(--accent)' }}>{wallets?.length ?? 0}</div>
        </div>
        <div className="card">
          <h3>Backfill Jobs</h3>
          <div className="stat-value" style={{ fontSize: 22 }}>{backfillJobs?.length ?? 0}</div>
          {runningJobs.length > 0 && (
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--yellow)', marginTop: 6, letterSpacing: '0.5px' }}>
              {runningJobs.length} running
            </div>
          )}
        </div>
      </div>

      <div className="card">
        <h3>Connection Details</h3>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
          <InfoRow label="RPC Endpoint" value={data?.rpc_url || 'https://api.devnet.solana.com'} mono />
          <InfoRow label="Network" value="DEVNET" />
          <InfoRow label="WebSocket" value={isOk ? 'Connected' : error ? 'Disconnected' : 'Connecting...'} valueColor={statusColor} />
          <InfoRow label="Last Slot" value={data?.last_slot?.toLocaleString() || '-'} mono />
        </div>
      </div>

      {backfillJobs && backfillJobs.length > 0 && (
        <div className="card" style={{ padding: 0 }}>
          <div style={{ padding: '18px 22px 0' }}>
            <h3 style={{ marginBottom: 0 }}>Backfill Jobs</h3>
          </div>
          <table className="data-table">
            <thead>
              <tr>
                <th>ID</th>
                <th>Wallet</th>
                <th>Status</th>
                <th>Fetched</th>
                <th>Indexed</th>
                <th>Created</th>
              </tr>
            </thead>
            <tbody>
              {backfillJobs.map((job) => (
                <tr key={job.id}>
                  <td className="mono" style={{ fontSize: 12 }}>{job.id}</td>
                  <td>
                    <span className="mono truncate" style={{ maxWidth: 120, fontSize: 12 }} title={job.wallet}>{job.wallet.slice(0, 12)}...</span>
                  </td>
                  <td>
                    <span
                      className="badge"
                      style={{
                        background: job.status === 'completed' ? 'var(--green-dim)' : job.status === 'running' ? 'var(--yellow-dim)' : job.status === 'failed' ? 'var(--red-dim)' : 'rgba(138, 146, 168, 0.1)',
                        color: job.status === 'completed' ? 'var(--green)' : job.status === 'running' ? 'var(--yellow)' : job.status === 'failed' ? 'var(--red)' : 'var(--text-secondary)',
                        border: `1px solid ${job.status === 'completed' ? 'rgba(34,197,94,0.12)' : job.status === 'running' ? 'rgba(255,170,0,0.12)' : job.status === 'failed' ? 'rgba(239,68,68,0.12)' : 'transparent'}`,
                      }}
                    >
                      {job.status}
                    </span>
                  </td>
                  <td className="mono" style={{ fontSize: 12 }}>{job.total_fetched}</td>
                  <td className="mono" style={{ fontSize: 12 }}>{job.total_indexed}</td>
                  <td style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--text-secondary)' }}>
                    {new Date(job.created_at).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

function InfoRow({ label, value, mono, valueColor }: { label: string; value: string; mono?: boolean; valueColor?: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '4px 0' }}>
      <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-muted)', letterSpacing: '1px', textTransform: 'uppercase' }}>{label}</span>
      <span style={{ fontFamily: mono ? 'var(--mono)' : 'var(--font)', fontSize: 13, color: valueColor || 'var(--text-primary)', fontWeight: 500 }}>{value}</span>
    </div>
  );
}
