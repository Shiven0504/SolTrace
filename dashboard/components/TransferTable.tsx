'use client';

import { useApi } from '@/hooks/useApi';

interface Transfer {
  signature: string;
  instruction_idx: number;
  block_time: string | null;
  direction: string;
  amount: number;
  mint: string | null;
  slot: number;
}

interface TransferTableProps {
  searchSig?: string | null;
}

export default function TransferTable({ searchSig }: TransferTableProps) {
  const listEndpoint = '/transfers?limit=50';
  const sigEndpoint = searchSig ? `/tx/${searchSig}` : null;

  const list = useApi<Transfer[]>(listEndpoint, { interval: 8000, autoFetch: !searchSig });
  const sig = useApi<Transfer[]>(sigEndpoint || '/health', { autoFetch: !!searchSig, public: true });

  const transfers = searchSig ? sig.data : list.data;
  const loading = searchSig ? sig.loading : list.loading;
  const error = searchSig ? sig.error : list.error;

  const formatTime = (iso: string | null) => {
    if (!iso) return '-';
    const d = new Date(iso);
    return d.toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit' });
  };

  const formatAmount = (lamports: number, mint: string | null) => {
    if (mint) return lamports.toLocaleString();
    const sol = lamports / 1_000_000_000;
    return sol >= 0.01 ? sol.toFixed(4) : sol.toFixed(9);
  };

  const solscanSig = (sig: string) => `https://solscan.io/tx/${sig}?cluster=devnet`;

  return (
    <div className="card" style={{ overflow: 'auto', padding: 0 }}>
      <div style={{ padding: '18px 22px 12px', display: 'flex', alignItems: 'center', gap: 10 }}>
        <h3 style={{ marginBottom: 0 }}>{searchSig ? `Results for ${searchSig.slice(0, 16)}...` : 'Recent Transfers'}</h3>
        {transfers && Array.isArray(transfers) && transfers.length > 0 && (
          <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '1px' }}>
            {transfers.length} entries
          </span>
        )}
      </div>

      {loading && <div className="empty-state">Loading...</div>}
      {error && <div className="error-msg" style={{ padding: '0 22px 16px' }}>{error}</div>}

      {transfers && Array.isArray(transfers) && transfers.length === 0 && (
        <div className="empty-state">No transfers found</div>
      )}

      {transfers && Array.isArray(transfers) && transfers.length > 0 && (
        <table className="data-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Signature</th>
              <th>Direction</th>
              <th>Amount</th>
              <th className="col-mint">Mint</th>
              <th className="col-slot">Slot</th>
            </tr>
          </thead>
          <tbody>
            {transfers.map((t) => (
              <tr key={`${t.signature}-${t.instruction_idx}`}>
                <td style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--text-secondary)' }}>
                  {formatTime(t.block_time)}
                </td>
                <td>
                  <a href={solscanSig(t.signature)} target="_blank" rel="noreferrer" className="mono truncate" title={t.signature} style={{ maxWidth: 120, fontSize: 12 }}>
                    {t.signature.slice(0, 12)}...
                  </a>
                </td>
                <td>
                  <span className={`badge ${t.direction}-bg`}>{t.direction}</span>
                </td>
                <td style={{ fontFamily: 'var(--mono)', fontSize: 12, fontWeight: 500 }}>
                  {formatAmount(t.amount, t.mint)}
                  {!t.mint && <span style={{ color: 'var(--text-muted)', marginLeft: 4, fontSize: 10, letterSpacing: '0.5px' }}>SOL</span>}
                </td>
                <td className="col-mint">
                  {t.mint ? (
                    <span className="mono truncate" title={t.mint} style={{ maxWidth: 80, fontSize: 12, color: 'var(--text-secondary)' }}>{t.mint.slice(0, 8)}...</span>
                  ) : (
                    <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '1px', textTransform: 'uppercase' }}>Native</span>
                  )}
                </td>
                <td className="col-slot" style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--text-muted)' }}>
                  {t.slot?.toLocaleString()}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
