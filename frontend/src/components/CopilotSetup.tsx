import { useState, useEffect } from 'react'
import { getCopilotStatus, disconnectCopilot } from '../lib/api'
import type { CopilotStatus } from '../lib/api'
import { useToast } from './Toast'

export function CopilotSetup() {
  const { showToast } = useToast()
  const [status, setStatus] = useState<CopilotStatus | null>(null)
  const [loading, setLoading] = useState(true)

  function loadStatus() {
    getCopilotStatus()
      .then(s => { setStatus(s); setLoading(false) })
      .catch(() => setLoading(false))
  }

  useEffect(() => {
    loadStatus()

    const params = new URLSearchParams(window.location.search)
    const copilotParam = params.get('copilot')
    if (copilotParam === 'connected') {
      showToast('GitHub Copilot connected', 'success')
      params.delete('copilot')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    } else if (copilotParam === 'error') {
      const message = params.get('message') || 'Connection failed'
      showToast(message, 'error')
      params.delete('copilot')
      params.delete('message')
      const newUrl = params.toString()
        ? `${window.location.pathname}?${params}`
        : window.location.pathname
      window.history.replaceState({}, '', newUrl)
    }
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  async function handleDisconnect() {
    try {
      await disconnectCopilot()
      showToast('GitHub Copilot disconnected', 'success')
      loadStatus()
    } catch (err) {
      showToast(
        'Failed to disconnect: ' + (err instanceof Error ? err.message : 'Unknown error'),
        'error',
      )
    }
  }

  function handleConnect() {
    window.location.href = '/_ui/api/copilot/connect'
  }

  if (loading) {
    return <div className="skeleton skeleton-block" role="status" aria-label="Loading Copilot status" />
  }

  return (
    <div className="card">
      <div className="card-header">
        <span className="card-title">{'> '}github copilot</span>
        {status?.connected && !status.expired && (
          <span className="tag-ok">CONNECTED</span>
        )}
        {status?.connected && status.expired && (
          <span className="tag-warn">EXPIRED</span>
        )}
        {!status?.connected && (
          <span className="tag-err">NOT CONNECTED</span>
        )}
      </div>
      {status?.connected && status.github_username && (
        <div className="copilot-info">
          <span className="copilot-username">{status.github_username}</span>
          {status.copilot_plan && (
            <span className="copilot-plan">{status.copilot_plan}</span>
          )}
        </div>
      )}
      <div className="kiro-actions">
        <button
          className="btn-save"
          type="button"
          onClick={handleConnect}
        >
          {status?.connected ? '$ reconnect' : '$ connect github'}
        </button>
        {status?.connected && (
          <button
            className="device-code-cancel"
            type="button"
            onClick={handleDisconnect}
          >
            disconnect
          </button>
        )}
      </div>
    </div>
  )
}
