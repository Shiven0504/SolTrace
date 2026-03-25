'use client';

import { useState, type FormEvent } from 'react';

interface SearchBarProps {
  onSearch: (sig: string | null) => void;
}

export default function SearchBar({ onSearch }: SearchBarProps) {
  const [query, setQuery] = useState('');

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    const trimmed = query.trim();
    onSearch(trimmed || null);
  };

  const handleClear = () => {
    setQuery('');
    onSearch(null);
  };

  return (
    <form onSubmit={handleSubmit} className="search-form" style={{ display: 'flex', gap: 8 }}>
      <div style={{ flex: 1, position: 'relative' }}>
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="var(--text-muted)"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          style={{ position: 'absolute', left: 12, top: '50%', transform: 'translateY(-50%)', pointerEvents: 'none' }}
        >
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <input
          className="input"
          style={{ width: '100%', paddingLeft: 36 }}
          placeholder="Search by transaction signature..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </div>
      <button type="submit" className="btn">Search</button>
      {query && (
        <button type="button" className="btn btn-ghost" onClick={handleClear}>Clear</button>
      )}
    </form>
  );
}
