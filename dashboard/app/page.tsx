import WelcomeBanner from '@/components/WelcomeBanner';
import StatsCards from '@/components/StatsCards';
import TransferChart from '@/components/TransferChart';
import TransferTable from '@/components/TransferTable';

export default function OverviewPage() {
  return (
    <div className="stagger">
      <WelcomeBanner />
      <StatsCards />
      <TransferChart />
      <TransferTable />
    </div>
  );
}
