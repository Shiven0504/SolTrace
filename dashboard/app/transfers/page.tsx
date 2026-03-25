'use client';

import { useState } from 'react';
import SearchBar from '@/components/SearchBar';
import TransferTable from '@/components/TransferTable';

export default function TransfersPage() {
  const [searchSig, setSearchSig] = useState<string | null>(null);

  return (
    <>
      <SearchBar onSearch={setSearchSig} />
      <TransferTable searchSig={searchSig} />
    </>
  );
}
