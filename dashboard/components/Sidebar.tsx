'use client';

import Image from 'next/image';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import logoSvg from '@/images/icon-option-2.svg';

interface NavItem {
  id: string;
  href: string;
  label: string;
  icon: React.ReactNode;
}

interface NavSection {
  section: string;
  items: NavItem[];
}

const navItems: NavSection[] = [
  {
    section: 'Dashboard',
    items: [
      {
        id: 'overview',
        href: '/',
        label: 'Overview',
        icon: (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="3" width="7" height="7" rx="1" />
            <rect x="14" y="3" width="7" height="7" rx="1" />
            <rect x="3" y="14" width="7" height="7" rx="1" />
            <rect x="14" y="14" width="7" height="7" rx="1" />
          </svg>
        ),
      },
      {
        id: 'transfers',
        href: '/transfers',
        label: 'Transfers',
        icon: (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="17 1 21 5 17 9" />
            <path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <polyline points="7 23 3 19 7 15" />
            <path d="M21 13v2a4 4 0 0 1-4 4H3" />
          </svg>
        ),
      },
      {
        id: 'wallets',
        href: '/wallets',
        label: 'Wallets',
        icon: (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M21 12V7H5a2 2 0 0 1 0-4h14v4" />
            <path d="M3 5v14a2 2 0 0 0 2 2h16v-5" />
            <path d="M18 12a2 2 0 0 0 0 4h4v-4h-4z" />
          </svg>
        ),
      },
    ],
  },
  {
    section: 'Monitoring',
    items: [
      {
        id: 'health',
        href: '/health',
        label: 'Indexer Health',
        icon: (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
            <path d="M22 12h-4l-3 9L9 3l-3 9H2" />
          </svg>
        ),
      },
    ],
  },
];

interface SidebarProps {
  isOpen: boolean;
  onClose: () => void;
}

export default function Sidebar({ isOpen, onClose }: SidebarProps) {
  const pathname = usePathname();

  const isActive = (href: string) => {
    if (href === '/') return pathname === '/';
    return pathname.startsWith(href);
  };

  return (
    <aside className={`sidebar${isOpen ? ' open' : ''}`}>
      {/* Brand */}
      <div className="sidebar-brand">
        <Link href="/" style={{ display: 'contents', textDecoration: 'none', color: 'inherit' }} onClick={onClose}>
          <Image src={logoSvg} alt="SolTrace" width={36} height={36} />
        </Link>
        <div>
          <Link href="/" style={{ textDecoration: 'none', color: 'inherit' }} onClick={onClose}>
            <h1
              style={{
                background: 'linear-gradient(135deg, var(--accent) 0%, var(--green) 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
                backgroundClip: 'text' as const,
                fontFamily: 'var(--font-display)',
              }}
            >
              Sol<span style={{ fontWeight: 700 }}>Trace</span>
            </h1>
          </Link>
          <p>Blockchain Indexer</p>
        </div>
      </div>

      {/* Nav sections */}
      {navItems.map((section) => (
        <div key={section.section}>
          <div className="sidebar-section">{section.section}</div>
          <nav className="sidebar-nav">
            {section.items.map((item) => (
              <Link
                key={item.id}
                href={item.href}
                onClick={onClose}
                style={{ textDecoration: 'none' }}
              >
                <button className={isActive(item.href) ? 'active' : ''}>
                  {item.icon}
                  {item.label}
                </button>
              </Link>
            ))}
          </nav>
        </div>
      ))}

      {/* Footer */}
      <div className="sidebar-footer">
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span
            style={{
              fontFamily: 'var(--mono)',
              fontSize: 9,
              color: 'var(--green)',
              background: 'var(--green-dim)',
              border: '1px solid rgba(34, 197, 94, 0.12)',
              padding: '2px 8px',
              borderRadius: 3,
              fontWeight: 500,
              textTransform: 'uppercase',
              letterSpacing: '1.5px',
            }}
          >
            devnet
          </span>
        </div>
      </div>
    </aside>
  );
}
