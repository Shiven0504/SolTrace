import type { Metadata } from 'next';
import WalletManager from '@/components/WalletManager';

export const metadata: Metadata = {
  title: 'Wallets',
};

export default function WalletsPage() {
  return <WalletManager />;
}
