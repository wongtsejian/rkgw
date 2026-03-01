import { useState } from 'react'
import { startOAuth, type OAuthDeviceResponse } from '../lib/api'
import { useToast } from '../components/Toast'
import { DeviceCodeDisplay } from '../components/DeviceCodeDisplay'

interface SetupProps {
  onComplete: () => void
}

type SetupStep = 'password' | 'oauth' | 'device-code'

export function Setup({ onComplete }: SetupProps) {
  const { showToast } = useToast()
  const [step, setStep] = useState<SetupStep>('password')
  const [apiKey, setApiKey] = useState('')
  const [region, setRegion] = useState('us-east-1')
  const [startUrl, setStartUrl] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [submitting, setSubmitting] = useState(false)
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [deviceAuth, setDeviceAuth] = useState<OAuthDeviceResponse | null>(null)

  function validatePassword(): boolean {
    const next: Record<string, string> = {}
    if (!apiKey.trim()) {
      next.apiKey = 'Required'
    } else if (apiKey.trim().length < 8) {
      next.apiKey = 'Must be at least 8 characters'
    }
    setErrors(next)
    return Object.keys(next).length === 0
  }

  function handleNext() {
    if (validatePassword()) setStep('oauth')
  }

  async function handleOAuth(flow: 'browser' | 'device') {
    setSubmitting(true)
    try {
      const result = await startOAuth({
        region,
        proxy_api_key: apiKey.trim(),
        flow,
        start_url: startUrl.trim() || undefined,
      })
      if (result.flow === 'browser') {
        window.open(result.authorize_url, '_blank', 'noopener,noreferrer')
      } else {
        setDeviceAuth(result)
        setStep('device-code')
      }
    } catch (err) {
      showToast(
        'Failed to start login: ' + (err instanceof Error ? err.message : 'Unknown error'),
        'error',
      )
    } finally {
      setSubmitting(false)
    }
  }

  function handleDeviceComplete() {
    showToast('Authentication successful! Redirecting...', 'success')
    setTimeout(onComplete, 600)
  }

  function handleDeviceError(message: string) {
    showToast(message, 'error')
    setDeviceAuth(null)
    setStep('oauth')
  }

  const labelStyle = {
    display: 'block' as const,
    fontSize: '0.78rem',
    fontWeight: 500,
    color: 'var(--text-secondary)',
    marginBottom: 6,
  }

  const hintStyle = {
    fontSize: '0.68rem',
    color: 'var(--text-tertiary)',
    marginTop: 6,
    display: 'block' as const,
    lineHeight: 1.4,
  }

  const errorStyle = {
    fontSize: '0.72rem',
    color: 'var(--red)',
    marginTop: 4,
    display: 'block' as const,
  }

  const logo = (
    <div className="auth-logo">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#101014" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 2L2 7l10 5 10-5-10-5z" />
        <path d="M2 17l10 5 10-5" />
        <path d="M2 12l10 5 10-5" />
      </svg>
    </div>
  )

  // --- Device Code Step ---
  if (step === 'device-code' && deviceAuth) {
    return (
      <div className="auth-overlay">
        <div className="auth-card" style={{ width: 440, textAlign: 'center' }}>
          {logo}
          <h2>Kiro Gateway</h2>
          <DeviceCodeDisplay
            userCode={deviceAuth.user_code}
            verificationUri={deviceAuth.verification_uri}
            verificationUriComplete={deviceAuth.verification_uri_complete}
            deviceCodeId={deviceAuth.device_code_id}
            onComplete={handleDeviceComplete}
            onError={handleDeviceError}
            onCancel={() => { setDeviceAuth(null); setStep('oauth') }}
          />
        </div>
      </div>
    )
  }

  // --- OAuth Step ---
  if (step === 'oauth') {
    return (
      <div className="auth-overlay">
        <div className="auth-card" style={{ width: 440, textAlign: 'left' }}>
          <div style={{ textAlign: 'center', marginBottom: 24 }}>
            {logo}
            <h2>Kiro Gateway</h2>
            <p style={{ margin: 0 }}>Sign in to your Kiro account</p>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
            {/* Start URL */}
            <div>
              <label htmlFor="setup-start-url" style={labelStyle}>
                Identity Center Start URL
                <span style={{ fontWeight: 400, color: 'var(--text-tertiary)', marginLeft: 6, fontSize: '0.68rem' }}>optional</span>
              </label>
              <input
                id="setup-start-url"
                className="auth-input"
                type="text"
                placeholder="https://your-org.awsapps.com/start/#/"
                value={startUrl}
                onChange={e => setStartUrl(e.target.value)}
                style={{ marginBottom: 0 }}
                autoFocus
              />
              <span style={hintStyle}>
                For IAM Identity Center (Pro license). Leave empty for AWS Builder ID (free).
              </span>
            </div>

            {/* Region */}
            <div>
              <label htmlFor="setup-region" style={labelStyle}>SSO Region</label>
              <select
                id="setup-region"
                className="auth-input"
                value={region}
                onChange={e => setRegion(e.target.value)}
                style={{ marginBottom: 0, cursor: 'pointer' }}
              >
                <option value="us-east-1">us-east-1</option>
                <option value="us-west-2">us-west-2</option>
                <option value="eu-west-1">eu-west-1</option>
                <option value="ap-southeast-1">ap-southeast-1</option>
              </select>
            </div>
          </div>

          {/* Login button */}
          <button
            className="auth-submit"
            type="button"
            disabled={submitting}
            onClick={() => handleOAuth('device')}
            style={{ marginTop: 28, opacity: submitting ? 0.7 : 1 }}
          >
            {submitting ? 'Starting...' : 'Login with Device Code'}
          </button>

          <button
            type="button"
            onClick={() => setStep('password')}
            style={{
              display: 'block',
              width: '100%',
              background: 'none',
              border: 'none',
              color: 'var(--text-tertiary)',
              fontSize: '0.72rem',
              fontFamily: 'var(--font-mono)',
              cursor: 'pointer',
              marginTop: 12,
              padding: '4px 0',
              transition: 'color 0.15s',
            }}
          >
            back
          </button>
        </div>
      </div>
    )
  }

  // --- Password Step (default) ---
  return (
    <div className="auth-overlay">
      <div className="auth-card" style={{ width: 440, textAlign: 'left' }}>
        <div style={{ textAlign: 'center', marginBottom: 24 }}>
          {logo}
          <h2>Kiro Gateway</h2>
          <p style={{ margin: 0 }}>Welcome! Let's get your gateway configured.</p>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 20 }}>
          {/* Gateway Password */}
          <div>
            <label htmlFor="setup-api-key" style={labelStyle}>Gateway Password</label>
            <div style={{ position: 'relative' }}>
              <input
                id="setup-api-key"
                className="auth-input"
                type={showPassword ? 'text' : 'password'}
                placeholder="Enter a password (min 8 characters)"
                autoComplete="new-password"
                value={apiKey}
                onChange={e => { setApiKey(e.target.value); setErrors(p => ({ ...p, apiKey: '' })) }}
                onKeyDown={e => { if (e.key === 'Enter') handleNext() }}
                style={{ marginBottom: 0, paddingRight: 60 }}
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowPassword(v => !v)}
                style={{
                  position: 'absolute',
                  right: 8,
                  top: '50%',
                  transform: 'translateY(-50%)',
                  background: 'none',
                  border: '1px solid var(--border)',
                  color: 'var(--text-tertiary)',
                  padding: '3px 8px',
                  borderRadius: 'var(--radius-sm)',
                  cursor: 'pointer',
                  fontSize: '0.65rem',
                  fontFamily: 'var(--font-mono)',
                }}
              >
                {showPassword ? 'hide' : 'show'}
              </button>
            </div>
            {errors.apiKey && <span style={errorStyle}>{errors.apiKey}</span>}
            <span style={hintStyle}>
              This password protects access to your gateway. You'll use it to authenticate API requests.
            </span>
          </div>
        </div>

        <button
          className="auth-submit"
          type="button"
          onClick={handleNext}
          style={{ marginTop: 28 }}
        >
          Next
        </button>
      </div>
    </div>
  )
}
