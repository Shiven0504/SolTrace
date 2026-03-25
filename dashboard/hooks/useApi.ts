'use client';

import { useState, useEffect, useCallback } from 'react';
import { useAuth } from './useAuth';

const BASE_URL = '/api';

interface UseApiOptions {
  autoFetch?: boolean;
  interval?: number | null;
  /** If true, fetch even without auth token (for public endpoints like /health) */
  public?: boolean;
}

interface UseApiReturn<T> {
  data: T | null;
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
}

export function useApi<T = unknown>(
  endpoint: string,
  options: UseApiOptions = {}
): UseApiReturn<T> {
  const { autoFetch = true, interval = null, public: isPublic = false } = options;
  const { token } = useAuth();
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(autoFetch);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback(async () => {
    const currentToken = typeof window !== 'undefined' ? localStorage.getItem('soltrace-token') : null;
    if (!isPublic && !currentToken) {
      setData(null);
      setLoading(false);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const headers: Record<string, string> = {};
      if (currentToken) headers.Authorization = `Bearer ${currentToken}`;
      const res = await fetch(`${BASE_URL}${endpoint}`, { headers });
      if (!res.ok) {
        const body = await res.json().catch(() => ({}));
        throw new Error(body.error || `HTTP ${res.status}`);
      }
      const json = await res.json();
      setData(json);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, [endpoint, isPublic]);

  // Clear user-scoped data immediately when token is removed (logout)
  useEffect(() => {
    if (!token && !isPublic) {
      setData(null);
      setError(null);
    }
  }, [token, isPublic]);

  useEffect(() => {
    if (autoFetch && (token || isPublic)) {
      fetchData();
    }
  }, [autoFetch, fetchData, token, isPublic]);

  useEffect(() => {
    if (!interval || (!token && !isPublic)) return;
    const id = setInterval(fetchData, interval);
    return () => clearInterval(id);
  }, [interval, fetchData, token, isPublic]);

  return { data, loading, error, refetch: fetchData };
}

export async function postApi<T = unknown>(endpoint: string, body: unknown): Promise<T> {
  const res = await fetch(`${BASE_URL}${endpoint}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...getAuthHeaders() },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || `HTTP ${res.status}`);
  }
  return res.json();
}

export async function deleteApi(endpoint: string): Promise<void> {
  const res = await fetch(`${BASE_URL}${endpoint}`, {
    method: 'DELETE',
    headers: { ...getAuthHeaders() },
  });
  if (!res.ok && res.status !== 204) {
    const data = await res.json().catch(() => ({}));
    throw new Error(data.error || `HTTP ${res.status}`);
  }
}

function getAuthHeaders(): Record<string, string> {
  const token = typeof window !== 'undefined' ? localStorage.getItem('soltrace-token') : null;
  return token ? { Authorization: `Bearer ${token}` } : {};
}
