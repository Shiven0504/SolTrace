import type { Metadata } from 'next';
import HealthBar from '@/components/HealthBar';

export const metadata: Metadata = {
  title: 'Health',
};

export default function HealthPage() {
  return <HealthBar />;
}
