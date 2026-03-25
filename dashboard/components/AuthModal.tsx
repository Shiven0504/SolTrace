'use client';

import { useState, useEffect, useRef, useCallback, type FormEvent } from 'react';
import { useAuth } from '@/hooks/useAuth';

/* Google Identity Services type declarations */
declare global {
  interface Window {
    google?: {
      accounts: {
        id: {
          initialize: (config: {
            client_id: string;
            callback: (response: { credential: string }) => void;
          }) => void;
          renderButton: (
            element: HTMLElement,
            config: {
              theme?: string;
              size?: string;
              width?: number;
              text?: string;
              shape?: string;
              logo_alignment?: string;
            }
          ) => void;
        };
      };
    };
  }
}

interface AuthModalProps {
  onClose: () => void;
}

type Step = 'auth' | 'pick-username';

export default function AuthModal({ onClose }: AuthModalProps) {
  const { login, register, googleLogin, setUsername, googleClientId } = useAuth();
  const [step, setStep] = useState<Step>('auth');
  const [mode, setMode] = useState<'login' | 'register'>('login');
  const [usernameField, setUsernameField] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const googleBtnRef = useRef<HTMLDivElement>(null);
  const [gsiLoaded, setGsiLoaded] = useState(false);

  const handleGoogleResponse = useCallback(async (response: { credential: string }) => {
    setError(null);
    setSubmitting(true);
    try {
      const result = await googleLogin(response.credential);
      if (result.is_new) {
        setStep('pick-username');
      } else {
        onClose();
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Google sign-in failed');
    } finally {
      setSubmitting(false);
    }
  }, [googleLogin, onClose]);

  useEffect(() => {
    if (!googleClientId) return;
    if (window.google?.accounts) {
      setGsiLoaded(true);
      return;
    }
    const script = document.createElement('script');
    script.src = 'https://accounts.google.com/gsi/client';
    script.async = true;
    script.defer = true;
    script.onload = () => setGsiLoaded(true);
    document.head.appendChild(script);
  }, [googleClientId]);

  useEffect(() => {
    if (!gsiLoaded || !googleClientId || !googleBtnRef.current || !window.google) return;
    window.google.accounts.id.initialize({
      client_id: googleClientId,
      callback: handleGoogleResponse,
    });
    window.google.accounts.id.renderButton(googleBtnRef.current, {
      theme: 'outline',
      size: 'large',
      width: 336,
      text: 'signin_with',
      shape: 'rectangular',
      logo_alignment: 'left',
    });
  }, [gsiLoaded, googleClientId, handleGoogleResponse]);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      if (mode === 'login') {
        await login(usernameField, password);
      } else {
        await register(usernameField, password);
      }
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setSubmitting(false);
    }
  };

  const handleSetUsername = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    setSubmitting(true);
    try {
      await setUsername(usernameField);
      onClose();
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to set username');
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0, 0, 0, 0.7)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        backdropFilter: 'blur(8px)',
        animation: 'fadeIn 0.2s ease',
      }}
      onClick={step === 'auth' ? onClose : undefined}
    >
      <div
        className="auth-modal-inner"
        style={{
          background: 'var(--bg-card)',
          border: '1px solid var(--border)',
          borderRadius: 8,
          padding: 32,
          width: '100%',
          maxWidth: 400,
          position: 'relative',
          animation: 'fadeUp 0.25s ease-out',
          boxShadow: '0 16px 64px rgba(0, 0, 0, 0.5)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Accent line at top */}
        <div style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          height: 1,
          borderRadius: '8px 8px 0 0',
          background: 'linear-gradient(90deg, var(--accent), var(--green))',
          opacity: 0.5,
        }} />

        {step === 'auth' && (
          <button
            onClick={onClose}
            style={{
              position: 'absolute',
              top: 14,
              right: 14,
              background: 'none',
              border: 'none',
              color: 'var(--text-muted)',
              fontSize: 18,
              cursor: 'pointer',
              lineHeight: 1,
              transition: 'color 0.15s',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--text-primary)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--text-muted)'; }}
          >
            &times;
          </button>
        )}

        {/* Step 1: Sign in */}
        {step === 'auth' && (
          <>
            <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 4, textAlign: 'center', letterSpacing: '-0.3px' }}>
              {mode === 'login' ? 'Welcome back' : 'Create account'}
            </h2>
            <p style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-muted)', marginBottom: 24, textAlign: 'center', letterSpacing: '0.5px' }}>
              {mode === 'login' ? 'Sign in to your SolTrace account' : 'Register a new SolTrace account'}
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
                <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Username or Email</label>
                <input className="input" style={{ width: '100%' }} placeholder="Enter username or email" value={usernameField} onChange={(e) => setUsernameField(e.target.value)} autoFocus={!googleClientId} />
              </div>
              <div>
                <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Password</label>
                <input className="input" style={{ width: '100%' }} type="password" placeholder={mode === 'register' ? 'Min 6 characters' : 'Enter password'} value={password} onChange={(e) => setPassword(e.target.value)} />
              </div>
              {error && (
                <div style={{ background: 'var(--red-dim)', color: 'var(--red)', border: '1px solid rgba(239,68,68,0.12)', padding: '8px 12px', borderRadius: 4, fontFamily: 'var(--mono)', fontSize: 12 }}>{error}</div>
              )}
              <button type="submit" className="btn" disabled={submitting || !usernameField.trim() || !password} style={{ width: '100%', padding: '11px 16px', marginTop: 4 }}>
                {submitting ? 'Please wait...' : mode === 'login' ? 'Sign in' : 'Create account'}
              </button>
            </form>

            <div style={{ textAlign: 'center', marginTop: 18, fontSize: 12, color: 'var(--text-secondary)' }}>
              {mode === 'login' ? (
                <>
                  Don&apos;t have an account?{' '}
                  <button onClick={() => { setMode('register'); setError(null); }} style={{ background: 'none', border: 'none', color: 'var(--accent)', cursor: 'pointer', fontWeight: 600, fontSize: 12, fontFamily: 'var(--mono)' }}>
                    Sign up
                  </button>
                </>
              ) : (
                <>
                  Already have an account?{' '}
                  <button onClick={() => { setMode('login'); setError(null); }} style={{ background: 'none', border: 'none', color: 'var(--accent)', cursor: 'pointer', fontWeight: 600, fontSize: 12, fontFamily: 'var(--mono)' }}>
                    Sign in
                  </button>
                </>
              )}
            </div>
          </>
        )}

        {/* Step 2: Pick a username */}
        {step === 'pick-username' && (
          <>
            <h2 style={{ fontSize: 20, fontWeight: 700, marginBottom: 4, textAlign: 'center', letterSpacing: '-0.3px' }}>
              Choose a username
            </h2>
            <p style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--text-muted)', marginBottom: 24, textAlign: 'center', letterSpacing: '0.5px' }}>
              Pick a display name for your SolTrace account
            </p>

            <form onSubmit={handleSetUsername} style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
              <div>
                <label style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--text-muted)', display: 'block', marginBottom: 6, letterSpacing: '1px', textTransform: 'uppercase' }}>Username</label>
                <input className="input" style={{ width: '100%' }} placeholder="3-32 characters" value={usernameField} onChange={(e) => setUsernameField(e.target.value)} autoFocus />
              </div>
              {error && (
                <div style={{ background: 'var(--red-dim)', color: 'var(--red)', border: '1px solid rgba(239,68,68,0.12)', padding: '8px 12px', borderRadius: 4, fontFamily: 'var(--mono)', fontSize: 12 }}>{error}</div>
              )}
              <button type="submit" className="btn" disabled={submitting || usernameField.trim().length < 3} style={{ width: '100%', padding: '11px 16px', marginTop: 4 }}>
                {submitting ? 'Please wait...' : 'Continue'}
              </button>
            </form>

            <button
              onClick={onClose}
              style={{ display: 'block', margin: '18px auto 0', background: 'none', border: 'none', color: 'var(--text-muted)', cursor: 'pointer', fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.5px' }}
              onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--text-secondary)'; }}
              onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--text-muted)'; }}
            >
              Skip for now
            </button>
          </>
        )}
      </div>
    </div>
  );
}
