import { createContext, useContext, useState, useEffect } from 'react'
import type { ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router-dom'
import { checkSetupStatus } from '../lib/api'
import type { User } from '../lib/api'

interface SessionContextValue {
  user: User
  setupComplete: boolean
}

const SessionContext = createContext<SessionContextValue | null>(null)

// eslint-disable-next-line react-refresh/only-export-components
export function useSession(): SessionContextValue {
  const ctx = useContext(SessionContext)
  if (!ctx) throw new Error('useSession must be used within SessionGate')
  return ctx
}

interface SessionGateProps {
  children: ReactNode
}

export function SessionGate({ children }: SessionGateProps) {
  const location = useLocation()
  const [state, setState] = useState<{
    loading: boolean
    user: User | null
    setupComplete: boolean
  }>({ loading: true, user: null, setupComplete: false })

  useEffect(() => {
    Promise.all([
      fetch('/_ui/api/auth/me', { credentials: 'include' })
        .then(res => res.ok ? res.json() as Promise<User> : null)
        .catch(() => null),
      checkSetupStatus(),
    ]).then(([user, setupComplete]) => {
      setState({ loading: false, user, setupComplete })
    })
  }, [])

  if (state.loading) {
    return (
      <div className="auth-overlay">
        <div role="status" aria-label="Loading session" style={{ color: 'var(--text-tertiary)', fontSize: '0.8rem', fontFamily: 'var(--font-mono)' }}>
          Loading...
        </div>
      </div>
    )
  }

  if (!state.user) {
    return <Navigate to="/login" replace />
  }

  const user = state.user

  // Forced password change redirect (skip if already on that page)
  if (user.must_change_password && location.pathname !== '/change-password') {
    return <Navigate to="/change-password" replace />
  }

  // Forced 2FA setup redirect for password users without TOTP (skip if already on that page)
  if (user.auth_method === 'password' && !user.totp_enabled && location.pathname !== '/setup-2fa') {
    return <Navigate to="/setup-2fa" replace />
  }

  return (
    <SessionContext.Provider value={{ user, setupComplete: state.setupComplete }}>
      {children}
    </SessionContext.Provider>
  )
}
