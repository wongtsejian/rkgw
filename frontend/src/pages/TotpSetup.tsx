import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { getTotpSetup, verifyTotpSetup } from '../lib/api'
import type { TotpSetupResponse } from '../lib/api'
import { useToast } from '../components/useToast'

type SetupStep = 'scan' | 'verify' | 'recovery'

export function TotpSetup() {
  const navigate = useNavigate()
  const { showToast } = useToast()
  const [step, setStep] = useState<SetupStep>('scan')
  const [setup, setSetup] = useState<TotpSetupResponse | null>(null)
  const [code, setCode] = useState('')
  const [loading, setLoading] = useState(true)
  const [submitting, setSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    getTotpSetup()
      .then(data => {
        setSetup(data)
        setLoading(false)
      })
      .catch(err => {
        setError(err instanceof Error ? err.message : 'Failed to load 2FA setup')
        setLoading(false)
      })
  }, [])

  async function handleVerify(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setSubmitting(true)
    try {
      await verifyTotpSetup(code)
      setStep('recovery')
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Verification failed')
    } finally {
      setSubmitting(false)
    }
  }

  function handleCopyCodes() {
    if (!setup) return
    const text = setup.recovery_codes.join('\n')
    navigator.clipboard.writeText(text)
      .then(() => showToast('Recovery codes copied', 'success'))
      .catch(() => showToast('Failed to copy codes', 'error'))
  }

  function handleDownloadCodes() {
    if (!setup) return
    const text = setup.recovery_codes.join('\n')
    const blob = new Blob([text], { type: 'text/plain' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'harbangan-recovery-codes.txt'
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  function handleDone() {
    navigate('/', { replace: true })
  }

  if (loading) {
    return (
      <div className="auth-overlay">
        <div role="status" aria-label="Loading 2FA setup" style={{ color: 'var(--text-tertiary)', fontSize: '0.8rem', fontFamily: 'var(--font-mono)' }}>
          Loading...
        </div>
      </div>
    )
  }

  if (!setup) {
    return (
      <div className="auth-overlay">
        <div className="auth-card">
          <div className="login-error" role="alert">{error || 'Failed to load 2FA setup'}</div>
        </div>
      </div>
    )
  }

  if (step === 'recovery') {
    return (
      <div className="auth-overlay">
        <div className="auth-card">
          <h2><span aria-hidden="true">{'> '}</span>RECOVERY CODES<span className="cursor" aria-hidden="true" /></h2>
          <p>save these codes in a secure location</p>

          <div className="recovery-codes">
            {setup.recovery_codes.map((c, i) => (
              <div key={i} className="recovery-codes-item">{c}</div>
            ))}
          </div>

          <div className="recovery-codes-actions">
            <button type="button" className="auth-submit" onClick={handleCopyCodes}>
              $ copy codes
            </button>
            <button type="button" className="auth-submit auth-submit-secondary" onClick={handleDownloadCodes}>
              $ download codes
            </button>
          </div>

          <button type="button" className="auth-submit" onClick={handleDone}>
            $ i&apos;ve saved my codes
          </button>
        </div>
      </div>
    )
  }

  return (
    <div className="auth-overlay">
      <div className="auth-card">
        <div className="auth-logo">
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="var(--bg)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
            <rect x="3" y="11" width="18" height="11" rx="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/>
          </svg>
        </div>
        <h2><span aria-hidden="true">{'> '}</span>SETUP 2FA<span className="cursor" aria-hidden="true" /></h2>

        <div className="step-indicator">
          <span className={step === 'scan' ? 'step-active' : 'step-done'}>[1] scan</span>
          <span className="step-sep"> &gt; </span>
          <span className={step === 'verify' ? 'step-active' : ''}>[2] verify</span>
        </div>

        {step === 'scan' && (
          <>
            <p>scan this QR code with your authenticator app</p>
            <div className="totp-qr">
              <img src={setup.qr_code_data_url} alt="TOTP QR code" width="200" height="200" />
            </div>
            <p style={{ fontSize: '0.65rem', color: 'var(--text-tertiary)' }}>or enter this key manually:</p>
            <div className="totp-secret">{setup.secret}</div>
            <button type="button" className="auth-submit" onClick={() => setStep('verify')}>
              $ next
            </button>
          </>
        )}

        {step === 'verify' && (
          <>
            <p>enter the 6-digit code from your authenticator</p>

            {error && (
              <div className="login-error" role="alert" aria-live="assertive">
                {error}
              </div>
            )}

            <form onSubmit={handleVerify}>
              <input
                className="auth-input totp-input"
                type="text"
                inputMode="numeric"
                autoComplete="one-time-code"
                placeholder="000000"
                maxLength={6}
                value={code}
                onChange={e => setCode(e.target.value)}
                autoFocus
                required
              />
              <button className="auth-submit" type="submit" disabled={submitting}>
                {submitting ? '$ verifying...' : '$ verify & activate'}
              </button>
            </form>

            <button
              type="button"
              className="auth-toggle-link"
              onClick={() => { setStep('scan'); setError(null) }}
            >
              back to QR code
            </button>
          </>
        )}
      </div>
    </div>
  )
}
