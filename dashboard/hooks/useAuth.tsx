'use client';

import { useState, useEffect, useCallback, createContext, useContext, type ReactNode } from 'react';

const BASE_URL = '/api';

interface User {
  username: string;
  email?: string;
  avatar_url?: string;
  [key: string]: unknown;
}

interface GoogleLoginResult {
  is_new: boolean;
}

interface AuthContextValue {
  user: User | null;
  token: string | null;
  loading: boolean;
  googleClientId: string | null;
  login: (username: string, password: string) => Promise<void>;
  register: (username: string, password: string) => Promise<void>;
  googleLogin: (credential: string) => Promise<GoogleLoginResult>;
  setUsername: (username: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [token, setToken] = useState<string | null>(() => {
    if (typeof window === 'undefined') return null;
    return localStorage.getItem('soltrace-token');
  });
  const [loading, setLoading] = useState(!!token);
  const [googleClientId, setGoogleClientId] = useState<string | null>(
    process.env.NEXT_PUBLIC_GOOGLE_CLIENT_ID || null
  );

  // Fetch Google client ID from backend as fallback
  useEffect(() => {
    if (googleClientId) return;
    fetch(`${BASE_URL}/auth/google-client-id`)
      .then((res) => res.json())
      .then((data) => {
        if (data.client_id) setGoogleClientId(data.client_id);
      })
      .catch(() => { /* Google OAuth not available */ });
  }, [googleClientId]);

  useEffect(() => {
    if (!token) {
      setLoading(false);
      return;
    }

    fetch(`${BASE_URL}/auth/me`, {
      headers: { Authorization: `Bearer ${token}` },
    })
      .then((res) => {
        if (!res.ok) throw new Error('invalid token');
        return res.json();
      })
      .then((data) => setUser(data))
      .catch(() => {
        localStorage.removeItem('soltrace-token');
        setToken(null);
        setUser(null);
      })
      .finally(() => setLoading(false));
  }, [token]);

  const login = useCallback(async (username: string, password: string) => {
    const res = await fetch(`${BASE_URL}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Login failed');
    localStorage.setItem('soltrace-token', data.token);
    setToken(data.token);
    setUser(data.user);
  }, []);

  const register = useCallback(async (username: string, password: string) => {
    const res = await fetch(`${BASE_URL}/auth/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Registration failed');
    localStorage.setItem('soltrace-token', data.token);
    setToken(data.token);
    setUser(data.user);
  }, []);

  const googleLogin = useCallback(async (credential: string): Promise<GoogleLoginResult> => {
    const res = await fetch(`${BASE_URL}/auth/google`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ credential }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Google sign-in failed');
    localStorage.setItem('soltrace-token', data.token);
    setToken(data.token);
    setUser(data.user);
    return { is_new: data.is_new ?? false };
  }, []);

  const setUsername = useCallback(async (username: string) => {
    const currentToken = localStorage.getItem('soltrace-token');
    const res = await fetch(`${BASE_URL}/auth/username`, {
      method: 'PUT',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${currentToken}`,
      },
      body: JSON.stringify({ username }),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || 'Failed to set username');
    setUser(data);
  }, []);

  const logout = useCallback(() => {
    localStorage.removeItem('soltrace-token');
    setToken(null);
    setUser(null);
  }, []);

  return (
    <AuthContext.Provider value={{ user, token, loading, googleClientId, login, register, googleLogin, setUsername, logout }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error('useAuth must be used within AuthProvider');
  return ctx;
}
