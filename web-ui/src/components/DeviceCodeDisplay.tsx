import { useState, useEffect, useCallback, useRef } from 'react'
import { pollDeviceCode } from '../lib/api'

interface DeviceCodeDisplayProps {
  userCode: string
  verificationUri: string
  verificationUriComplete: string
  deviceCodeId: string
  onComplete: () => void
  onError: (message: string) => void
  onCancel: () => void
}

export function DeviceCodeDisplay({
  userCode,
  verificationUri,
  verificationUriComplete,
  deviceCodeId,
  onComplete,
  onError,
  onCancel,
}: DeviceCodeDisplayProps) {
  const [copied, setCopied] = useState(false)
  const pollRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pollIntervalRef = useRef(5)
  const mountedRef = useRef(true)

  const stopPolling = useCallback(() => {
    if (pollRef.current) clearTimeout(pollRef.current)
  }, [])

  const poll = useCallback(async () => {
    if (!mountedRef.current) return
    try {
      const result = await pollDeviceCode(deviceCodeId)
      if (!mountedRef.current) return
      if (result.status === 'complete') {
        stopPolling()
        onComplete()
      } else if (result.status === 'slow_down') {
        pollIntervalRef.current = 10
        pollRef.current = setTimeout(poll, pollIntervalRef.current * 1000)
      } else {
        pollRef.current = setTimeout(poll, pollIntervalRef.current * 1000)
      }
    } catch (err) {
      if (!mountedRef.current) return
      stopPolling()
      onError(err instanceof Error ? err.message : 'Polling failed')
    }
  }, [deviceCodeId, stopPolling, onComplete, onError])

  useEffect(() => {
    mountedRef.current = true
    pollRef.current = setTimeout(poll, pollIntervalRef.current * 1000)
    return () => {
      mountedRef.current = false
      stopPolling()
    }
  }, [poll, stopPolling])

  async function copyCode() {
    try {
      await navigator.clipboard.writeText(userCode)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      // Fallback: ignore
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 20 }}>
      <p style={{ fontSize: '0.8rem', color: 'var(--text-tertiary)', margin: 0, lineHeight: 1.5 }}>
        Enter this code when prompted
      </p>

      {/* User code display */}
      <button
        type="button"
        onClick={copyCode}
        style={{
          background: 'rgba(16, 16, 20, 0.6)',
          border: '1px solid var(--amber-dim)',
          borderRadius: 'var(--radius)',
          padding: '16px 32px',
          cursor: 'pointer',
          transition: 'all 0.2s',
        }}
        aria-label={`Copy code ${userCode}`}
      >
        <div style={{
          fontFamily: 'var(--font-mono)',
          fontSize: '1.6rem',
          fontWeight: 700,
          letterSpacing: '0.15em',
          color: 'var(--amber)',
        }}>
          {userCode}
        </div>
        <div style={{
          fontSize: '0.65rem',
          color: copied ? 'var(--green)' : 'var(--text-tertiary)',
          fontFamily: 'var(--font-mono)',
          marginTop: 6,
          transition: 'color 0.2s',
        }}>
          {copied ? 'copied!' : 'click to copy'}
        </div>
      </button>

      {/* Verification link */}
      <a
        href={verificationUriComplete}
        target="_blank"
        rel="noopener noreferrer"
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: 6,
          fontSize: '0.78rem',
          color: 'var(--blue)',
          textDecoration: 'none',
          fontFamily: 'var(--font-mono)',
          padding: '8px 16px',
          borderRadius: 'var(--radius-sm)',
          border: '1px solid var(--border)',
          background: 'var(--blue-dim)',
          transition: 'all 0.15s',
        }}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" />
          <polyline points="15 3 21 3 21 9" />
          <line x1="10" y1="14" x2="21" y2="3" />
        </svg>
        Open verification page
      </a>
      <span style={{ fontSize: '0.65rem', color: 'var(--text-tertiary)', fontFamily: 'var(--font-mono)' }}>
        {verificationUri}
      </span>

      {/* Waiting indicator */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        fontSize: '0.72rem',
        color: 'var(--text-tertiary)',
      }}>
        <div style={{
          width: 6,
          height: 6,
          borderRadius: '50%',
          background: 'var(--amber)',
          animation: 'emptyPulse 1.5s ease-in-out infinite',
        }} />
        Waiting for authorization...
      </div>

      <button
        type="button"
        onClick={() => { stopPolling(); onCancel() }}
        style={{
          background: 'none',
          border: 'none',
          color: 'var(--text-tertiary)',
          fontSize: '0.72rem',
          fontFamily: 'var(--font-mono)',
          cursor: 'pointer',
          padding: '4px 8px',
          transition: 'color 0.15s',
        }}
      >
        cancel
      </button>
    </div>
  )
}
