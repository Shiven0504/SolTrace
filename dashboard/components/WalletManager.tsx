'use client';

import { useState, type FormEvent } from 'react';
import { useApi, postApi, deleteApi } from '@/hooks/useApi';
import { useAuth } from '@/hooks/useAuth';

interface Wallet {
  wallet_pubkey: string;
  label: string | null;
  created_at: string;
}

interface Balance {
  balance: number;
  mint: string | null;
}

interface Transfer {
  signature: string;
  instruction_idx: number;
  direction: string;
  amount: number;
  block_time: string | null;
}

export default function WalletManager() {
  const { user } = useAuth();
  const { data: wallets, loading, error, refetch } = useApi<Wallet[]>('/wallets');
  const [pubkey, setPubkey] = useState('');
  const [label, setLabel] = useState('');
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [backfilling, setBackfilling] = useState<string | null>(null);

  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);

  const handleDelete = async (pubkey: string) => {
    setDeleting(pubkey);
    setConfirmDelete(null);
    try {
      await deleteApi(`/wallets/${pubkey}`);
      refetch();
    } catch {
      // ignore
    } finally {
      setDeleting(null);
    }
  };

  const handleBackfill = async (pubkey: string) => {
    setBackfilling(pubkey);
    try {
      await postApi('/backfill', { wallet: pubkey });
    } catch {
      // ignore
    } finally {
      setBackfilling(null);
    }
  };

  const handleAdd = async (e: FormEvent) => {
    e.preventDefault();
    if (!pubkey.trim()) return;
    setAdding(true);
    setAddError(null);
    try {
      await postApi('/wallets', { pubkey: pubkey.trim(), label: label.trim() || null });
      setPubkey('');
      setLabel('');
      refetch();
    } catch (err: unknown) {
      setAddError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setAdding(false);
    }
  };

  return (
    <div className="stagger" style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
      <div className="card">
        <h3>Add Wallet to Watch</h3>
        {user ? (
          <>
            <form onSubmit={handleAdd} className="form-row" style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
              <input className="input" placeholder="Wallet pubkey..." value={pubkey} onChange={(e) => setPubkey(e.target.value)} style={{ flex: 2, minWidth: 200 }} />
              <input className="input" placeholder="Label (optional)" value={label} onChange={(e) => setLabel(e.target.value)} style={{ flex: 1, minWidth: 120 }} />
              <button type="submit" className="btn" disabled={adding || !pubkey.trim()}>
                {adding ? 'Adding...' : 'Watch'}
              </button>
            </form>
            {addError && <div className="error-msg" style={{ marginTop: 8 }}>{addError}</div>}
          </>
        ) : (
          <div className="empty-state" style={{ fontSize: 13 }}>
            <a href="/login" style={{ color: 'var(--accent)', textDecoration: 'underline' }}>Sign in</a> to add and manage wallets
          </div>
        )}
      </div>

      <div className="card">
        <h3>Watched Wallets</h3>
        {loading && <div className="empty-state">Loading...</div>}
        {error && <div className="error-msg">{error}</div>}
        {wallets && wallets.length === 0 && <div className="empty-state">No wallets being watched yet</div>}
        {wallets && wallets.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {wallets.map((w) => (
              <WalletCard
                key={w.wallet_pubkey}
                wallet={w}
                expanded={expanded === w.wallet_pubkey}
                onToggle={() => setExpanded(expanded === w.wallet_pubkey ? null : w.wallet_pubkey)}
                onDelete={() => setConfirmDelete(w.wallet_pubkey)}
                confirmingDelete={confirmDelete === w.wallet_pubkey}
                onConfirmDelete={() => handleDelete(w.wallet_pubkey)}
                onCancelDelete={() => setConfirmDelete(null)}
                onBackfill={() => handleBackfill(w.wallet_pubkey)}
                deleting={deleting === w.wallet_pubkey}
                backfilling={backfilling === w.wallet_pubkey}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface WalletCardProps {
  wallet: Wallet;
  expanded: boolean;
  onToggle: () => void;
  onDelete: () => void;
  onBackfill: () => void;
  deleting: boolean;
  backfilling: boolean;
  confirmingDelete: boolean;
  onConfirmDelete: () => void;
  onCancelDelete: () => void;
}

function WalletCard({ wallet, expanded, onToggle, onDelete, onBackfill, deleting, backfilling, confirmingDelete, onConfirmDelete, onCancelDelete }: WalletCardProps) {
  const { data: balances } = useApi<Balance[]>(`/balances?wallet=${wallet.wallet_pubkey}`, { autoFetch: expanded });
  const { data: transfers } = useApi<Transfer[]>(`/transfers?wallet=${wallet.wallet_pubkey}&limit=10`, { autoFetch: expanded });

  const formatTime = (iso: string | null) => {
    if (!iso) return '';
    return new Date(iso).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  };

  return (
    <div
      style={{
        background: expanded ? 'var(--bg-card-hover)' : 'var(--bg-secondary)',
        border: `1px solid ${expanded ? 'var(--border-glow)' : 'var(--border)'}`,
        borderRadius: 6,
        overflow: 'hidden',
        transition: 'all 0.2s ease',
      }}
    >
      <button
        onClick={onToggle}
        style={{
          width: '100%',
          display: 'flex',
          alignItems: 'center',
          gap: 12,
          padding: '12px 16px',
          background: 'none',
          border: 'none',
          color: 'var(--text-primary)',
          textAlign: 'left',
          cursor: 'pointer',
          transition: 'background 0.15s',
        }}
      >
        <span style={{
          transform: expanded ? 'rotate(90deg)' : 'none',
          transition: '0.2s ease',
          fontSize: 10,
          color: expanded ? 'var(--accent)' : 'var(--text-muted)',
        }}>
          &#9654;
        </span>
        <span className="mono" style={{ flex: 1, fontSize: 12 }}>{wallet.wallet_pubkey}</span>
        {wallet.label && (
          <span style={{
            fontFamily: 'var(--mono)',
            background: 'var(--accent-dim)',
            border: '1px solid var(--border)',
            padding: '2px 8px',
            borderRadius: 3,
            fontSize: 10,
            color: 'var(--accent)',
            letterSpacing: '0.5px',
          }}>
            {wallet.label}
          </span>
        )}
        <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', letterSpacing: '0.3px' }}>{formatTime(wallet.created_at)}</span>
        <button
          onClick={(e) => { e.stopPropagation(); onBackfill(); }}
          disabled={backfilling}
          title="Backfill historical transactions"
          style={{
            background: 'none', border: '1px solid var(--border)', borderRadius: 4,
            cursor: backfilling ? 'wait' : 'pointer', padding: '4px 8px',
            color: backfilling ? 'var(--text-muted)' : 'var(--accent)',
            fontFamily: 'var(--mono)', fontSize: 9, letterSpacing: '0.5px',
            transition: 'all 0.15s',
          }}
        >
          {backfilling ? 'Syncing...' : 'Backfill'}
        </button>
        <button
          onClick={(e) => { e.stopPropagation(); onDelete(); }}
          disabled={deleting}
          title="Remove wallet"
          style={{
            background: 'none', border: 'none', cursor: deleting ? 'wait' : 'pointer',
            padding: '4px 6px', color: 'var(--text-muted)', lineHeight: 1,
            transition: 'color 0.15s', display: 'flex', alignItems: 'center',
          }}
          onMouseEnter={(e) => (e.currentTarget.style.color = 'var(--red)')}
          onMouseLeave={(e) => (e.currentTarget.style.color = 'var(--text-muted)')}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
            <line x1="10" y1="11" x2="10" y2="17" />
            <line x1="14" y1="11" x2="14" y2="17" />
          </svg>
        </button>
      </button>

      {confirmingDelete && (
        <div style={{
          display: 'flex', alignItems: 'center', justifyContent: 'space-between',
          padding: '10px 16px', background: 'var(--bg-surface)',
          borderTop: '1px solid var(--border)',
          animation: 'fadeUp 0.15s ease-out',
        }}>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-secondary)', letterSpacing: '0.3px' }}>
            Remove this wallet?
          </span>
          <div style={{ display: 'flex', gap: 8 }}>
            <button
              onClick={onCancelDelete}
              style={{
                background: 'none', border: '1px solid var(--border)', borderRadius: 4,
                padding: '4px 12px', cursor: 'pointer', fontFamily: 'var(--mono)',
                fontSize: 10, color: 'var(--text-muted)', letterSpacing: '0.5px',
              }}
            >
              Cancel
            </button>
            <button
              onClick={onConfirmDelete}
              style={{
                background: 'var(--red-dim)', border: '1px solid rgba(239,68,68,0.2)', borderRadius: 4,
                padding: '4px 12px', cursor: 'pointer', fontFamily: 'var(--mono)',
                fontSize: 10, color: 'var(--red)', letterSpacing: '0.5px', fontWeight: 600,
              }}
            >
              Remove
            </button>
          </div>
        </div>
      )}

      {expanded && (
        <div style={{ padding: '0 16px 16px', borderTop: '1px solid var(--border-subtle)', animation: 'fadeUp 0.2s ease-out' }}>
          <div style={{ marginTop: 14 }}>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--text-muted)', marginBottom: 8, letterSpacing: '2px', textTransform: 'uppercase' }}>Balances</div>
            {balances && balances.length > 0 ? (
              <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
                {balances.map((b, i) => (
                  <div key={i} style={{
                    background: 'var(--bg-surface)',
                    border: '1px solid var(--border)',
                    padding: '8px 12px',
                    borderRadius: 4,
                    fontSize: 12,
                  }}>
                    <span className="mono" style={{ fontWeight: 600, color: 'var(--accent)' }}>{(b.balance / 1_000_000_000).toFixed(4)}</span>
                    <span style={{ fontFamily: 'var(--mono)', color: 'var(--text-muted)', marginLeft: 6, fontSize: 10, letterSpacing: '0.5px' }}>{b.mint ? b.mint.slice(0, 6) + '...' : 'SOL'}</span>
                  </div>
                ))}
              </div>
            ) : (
              <div style={{ fontFamily: 'var(--mono)', color: 'var(--text-muted)', fontSize: 11, letterSpacing: '0.5px' }}>No balances</div>
            )}
          </div>

          <div style={{ marginTop: 16 }}>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--text-muted)', marginBottom: 8, letterSpacing: '2px', textTransform: 'uppercase' }}>Recent Transfers</div>
            {transfers && transfers.length > 0 ? (
              transfers.map((t) => (
                <div key={`${t.signature}-${t.instruction_idx}`} style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: 8,
                  padding: '7px 0',
                  borderBottom: '1px solid var(--border-subtle)',
                  fontSize: 12,
                }}>
                  <span className={`badge ${t.direction}-bg`} style={{ fontSize: 9 }}>{t.direction}</span>
                  <span className="mono" style={{ color: 'var(--text-secondary)', fontWeight: 500 }}>{(t.amount / 1_000_000_000).toFixed(4)}</span>
                  <span className="mono truncate" style={{ flex: 1, maxWidth: 100, color: 'var(--text-muted)', fontSize: 11 }}>{t.signature.slice(0, 12)}...</span>
                  <span style={{ fontFamily: 'var(--mono)', color: 'var(--text-muted)', fontSize: 10 }}>{formatTime(t.block_time)}</span>
                </div>
              ))
            ) : (
              <div style={{ fontFamily: 'var(--mono)', color: 'var(--text-muted)', fontSize: 11, letterSpacing: '0.5px' }}>No transfers</div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
