'use client';

import { useApi } from '@/hooks/useApi';

interface Transfer {
  direction: string;
  amount: number;
}

interface Wallet {
  wallet_pubkey: string;
}

export default function StatsCards() {
  const { data: transfers } = useApi<Transfer[]>('/transfers?limit=1000', { interval: 10000 });
  const { data: wallets } = useApi<Wallet[]>('/wallets', { interval: 10000 });

  const deposits = transfers?.filter((t) => t.direction === 'deposit') ?? [];
  const withdrawals = transfers?.filter((t) => t.direction === 'withdrawal') ?? [];

  const totalDepositAmount = deposits.reduce((sum, t) => sum + t.amount, 0);
  const totalWithdrawalAmount = withdrawals.reduce((sum, t) => sum + t.amount, 0);

  const formatSol = (lamports: number) => {
    if (lamports === 0) return '0';
    const sol = lamports / 1_000_000_000;
    if (sol >= 1) return sol.toFixed(2);
    return sol.toFixed(6);
  };

  const cards = [
    { label: 'Watched Wallets', value: wallets?.length ?? 0, color: 'var(--accent)', glow: 'var(--accent-glow)' },
    { label: 'Total Transfers', value: transfers?.length ?? 0, color: 'var(--text-primary)', glow: 'transparent' },
    { label: 'Deposits', value: deposits.length, sub: `${formatSol(totalDepositAmount)} SOL`, color: 'var(--green)', glow: 'var(--green-glow)' },
    { label: 'Withdrawals', value: withdrawals.length, sub: `${formatSol(totalWithdrawalAmount)} SOL`, color: 'var(--red)', glow: 'var(--red-glow)' },
  ];

  return (
    <div className="stats-grid">
      {cards.map((card) => (
        <div className="card" key={card.label} style={{ padding: '24px 20px' }}>
          <h3>{card.label}</h3>
          <div
            className="stat-value"
            style={{
              color: card.color,
              textShadow: card.glow !== 'transparent' ? `0 0 20px ${card.glow}` : 'none',
            }}
          >
            {card.value}
          </div>
          {card.sub && <div className="stat-sub">{card.sub}</div>}
        </div>
      ))}
    </div>
  );
}
