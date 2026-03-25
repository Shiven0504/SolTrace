'use client';

import { useState, useEffect, useRef, useCallback, type FormEvent } from 'react';
import { useRouter } from 'next/navigation';
import Link from 'next/link';
import Image from 'next/image';
import { useAuth } from '@/hooks/useAuth';
import logoSvg from '@/images/icon-option-2.svg';

declare global {
  interface Window {
    google?: {
      accounts: {
        id: {
          initialize: (config: { client_id: string; callback: (response: { credential: string }) => void }) => void;
          renderButton: (element: HTMLElement, config: { theme?: string; size?: string; width?: number; text?: string; shape?: string; logo_alignment?: string }) => void;
        };
      };
    };
  }
}

export default function SignupPage() {
  const router = useRouter();
  const { user, register, googleLogin, googleClientId, setUsername: setUsernameApi } = useAuth();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const googleBtnRef = useRef<HTMLDivElement>(null);
  const [gsiLoaded, setGsiLoaded] = useState(false);
  const [pickUsername, setPickUsername] = useState(false);
  const [newUsername, setNewUsername] = useState('');

  // Redirect if already logged in
  useEffect(() => {
    if (user) router.replace('/');
  }, [user, router]);

  const handleGoogleResponse = useCallback(async (response: { credential: string }) => {
    setError(null);
    setSubmitting(true);
    try {
      const result = await googleLogin(response.credential);
      if (result.is_new) {
        setPickUsername(true);
      } else {
        router.replace('/');
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Google sign-up failed');
    } finally {
      setSubmitting(false);
    }
  }, [googleLogin, router]);

  useEffect(() => {
    if (!googleClientId) return;
    if (window.google?.accounts) { setGsiLoaded(true); return; }
    const script = document.createElement('script');
    script.src = 'https://accounts.google.com/gsi/client';
    script.async = true;
    script.defer = true;
    script.onload = () => setGsiLoaded(true);
    document.head.appendChild(script);
  }, [googleClientId]);

  useEffect(() => {
    if (!gsiLoaded || !googleClientId || !googleBtnRef.current || !window.google) return;
    window.google.accounts.id.initialize({ client_id: googleClientId, callback: handleGoogleResponse });
    window.google.accounts.id.renderButton(googleBtnRef.current, { theme: 'outline', size: 'large', width: 336, text: 'signup_with', shape: 'rectangular', logo_alignment: 'left' });
  }, [gsiLoaded, googleClientId, handleGoogleResponse]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await register(username, password);
      router.replace('/');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Registration failed');
    } finally {
      setSubmitting(false);
    }
  };

  const handleSetUsername = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await setUsernameApi(newUsername);
      router.replace('/');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to set username');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div style={{ minHeight: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 20, position: 'relative', zIndex: 2 }}>
      <div style={{ width: '100%', maxWidth: 400 }}>
        {/* Brand */}
        <Link href="/" style={{ display: 'flex', alignItems: 'center', gap: 10, justifyContent: 'center', marginBottom: 36, textDecoration: 'none', color: 'inherit' }}>
          <Image src={logoSvg} alt="SolTrace" width={32} height={32} />
          <span style={{
            fontFamily: 'var(--mono)',
            fontSize: 20,
            fontWeight: 700,
            letterSpacing: '-0.5px',
            background: 'linear-gradient(135deg, var(--accent) 0%, var(--green) 100%)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
            backgroundClip: 'text',
          }}>
            SolTrace
          </span>
        </Link>

        {/* Card */}
        <div
          style={{
            background: 'var(--bg-card)',
            border: '1px solid var(--border)',
            borderRadius: 8,
            padding: 32,
            position: 'relative',
            overflow: 'hidden',
          }}
        >
          {/* Accent line */}
          <div style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 1, background: 'linear-gradient(90deg, var(--accent), var(--green))', opacity: 0.5 }} />

          {/* Close button */}
          <button
            onClick={() => router.push('/')}
            aria-label="Close"
            style={{
              position: 'absolute', top: 12, right: 12,
              background: 'none', border: 'none', cursor: 'pointer',
              color: 'var(--text-muted)', padding: 4, lineHeight: 1,
              fontSize: 18, fontWeight: 300, transition: 'color 0.15s',
            }}
            onMouseEnter={(e) => (e.currentTarget.style.color = 'var(--text-primary)')}
            onMouseLeave={(e) => (e.currentTarget.style.color = 'var(--text-muted)')}
          >
            &#x2715;
          </button>

          {!pickUsername ? (
            <>
              <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4, letterSpacing: '-0.3px' }}>Sign up</h1>
              <p style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-muted)', marginBottom: 24, letterSpacing: '0.5px' }}>
                Create your SolTrace account
              </p>

              {googleClientId && (
                <>
                  <div style={{ display: 'flex', justifyContent: 'center', marginBottom: 16 }}>
                    <div ref={googleBtnRef} />
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 16 }}>
                    <div style={{ flex: 1, height: 1, background: 'var(--border)' }} />
                    <span style={{ fontFamily: 'var(--mono)', fontSize: 9, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '2px' }}>or</span>
                    <div style={{ flex: 1, height: 1, background: 'var(--border)' }} />
                  </div>
                </>
              )}

              <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                <div>
                  <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Username</label>
                  <input className="input" style={{ width: '100%' }} placeholder="Choose a username" value={username} onChange={(e) => setUsername(e.target.value)} autoFocus={!googleClientId} />
                </div>
                <div>
                  <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Password</label>
                  <input className="input" style={{ width: '100%' }} type="password" placeholder="Min 6 characters" value={password} onChange={(e) => setPassword(e.target.value)} />
                </div>
                {error && (
                  <div style={{ background: 'var(--red-dim)', color: 'var(--red)', border: '1px solid rgba(239,68,68,0.12)', padding: '8px 12px', borderRadius: 4, fontFamily: 'var(--mono)', fontSize: 12 }}>{error}</div>
                )}
                <button type="submit" className="btn" disabled={submitting || !username.trim() || !password || password.length < 6} style={{ width: '100%', padding: '11px 16px', marginTop: 4 }}>
                  {submitting ? 'Creating account...' : 'Create account'}
                </button>
              </form>

              <div style={{ textAlign: 'center', marginTop: 20, fontSize: 12, color: 'var(--text-secondary)' }}>
                Already have an account?{' '}
                <Link href="/login" style={{ color: 'var(--accent)', fontWeight: 600, fontFamily: 'var(--mono)', fontSize: 12 }}>
                  Sign in
                </Link>
              </div>
            </>
          ) : (
            <>
              <h1 style={{ fontSize: 22, fontWeight: 700, marginBottom: 4, letterSpacing: '-0.3px' }}>Choose a username</h1>
              <p style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-muted)', marginBottom: 24, letterSpacing: '0.5px' }}>
                Pick a display name for your account
              </p>

              <form onSubmit={handleSetUsername} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
                <div>
                  <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Username</label>
                  <input className="input" style={{ width: '100%' }} placeholder="3-32 characters" value={newUsername} onChange={(e) => setNewUsername(e.target.value)} autoFocus />
                </div>
                {error && (
                  <div style={{ background: 'var(--red-dim)', color: 'var(--red)', border: '1px solid rgba(239,68,68,0.12)', padding: '8px 12px', borderRadius: 4, fontFamily: 'var(--mono)', fontSize: 12 }}>{error}</div>
                )}
                <button type="submit" className="btn" disabled={submitting || newUsername.trim().length < 3} style={{ width: '100%', padding: '11px 16px', marginTop: 4 }}>
                  {submitting ? 'Please wait...' : 'Continue'}
                </button>
              </form>

              <button
                onClick={() => router.replace('/')}
                style={{ display: 'block', margin: '18px auto 0', background: 'none', border: 'none', color: 'var(--text-muted)', cursor: 'pointer', fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.5px' }}
              >
                Skip for now
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
