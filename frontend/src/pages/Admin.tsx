import { useState } from 'react'
import { UserTable } from '../components/UserTable'
import { DomainManager } from '../components/DomainManager'
import { useSession } from '../components/SessionGate'
import { useToast } from '../components/useToast'
import { adminCreateUser } from '../lib/api'

export function Admin() {
  const { setupComplete } = useSession()
  const { showToast } = useToast()
  const [newEmail, setNewEmail] = useState('')
  const [newName, setNewName] = useState('')
  const [newPassword, setNewPassword] = useState('')
  const [newRole, setNewRole] = useState<'admin' | 'user'>('user')
  const [creating, setCreating] = useState(false)
  const [refreshKey, setRefreshKey] = useState(0)

  async function handleCreateUser(e: React.FormEvent) {
    e.preventDefault()
    if (!newEmail || !newName || !newPassword) return
    setCreating(true)
    try {
      await adminCreateUser(newEmail, newName, newPassword, newRole)
      showToast(`User ${newEmail} created`, 'success')
      setNewEmail('')
      setNewName('')
      setNewPassword('')
      setNewRole('user')
      setRefreshKey(k => k + 1)
    } catch (err) {
      showToast(err instanceof Error ? err.message : 'Failed to create user', 'error')
    } finally {
      setCreating(false)
    }
  }

  return (
    <>
      {!setupComplete && (
        <div className="setup-banner">
          <div className="setup-banner-icon">!</div>
          <div>
            <strong>Welcome, admin!</strong> Your gateway is almost ready.
            Add your organization's domain below to restrict who can sign in.
            Leave empty to allow any Google account.
          </div>
        </div>
      )}

      <h2 className="section-header">DOMAIN ALLOWLIST</h2>
      <div className="mb-24">
        <DomainManager />
      </div>

      <h2 className="section-header">CREATE PASSWORD USER</h2>
      <div className="card mb-24">
        <form onSubmit={handleCreateUser}>
          <div className="create-user-form">
            <div className="create-user-row">
              <input
                className="config-input"
                type="email"
                placeholder="email"
                value={newEmail}
                onChange={e => setNewEmail(e.target.value)}
                required
              />
              <input
                className="config-input"
                type="text"
                placeholder="name"
                value={newName}
                onChange={e => setNewName(e.target.value)}
                required
              />
            </div>
            <div className="create-user-row">
              <input
                className="config-input"
                type="password"
                placeholder="password (min 8 chars)"
                value={newPassword}
                onChange={e => setNewPassword(e.target.value)}
                minLength={8}
                required
              />
              <select
                className="config-input"
                value={newRole}
                onChange={e => setNewRole(e.target.value as 'admin' | 'user')}
              >
                <option value="user">user</option>
                <option value="admin">admin</option>
              </select>
            </div>
            <div>
              <button type="submit" className="btn-save" disabled={creating}>
                {creating ? 'Creating...' : 'Create User'}
              </button>
            </div>
          </div>
        </form>
      </div>

      <h2 className="section-header">USER MANAGEMENT</h2>
      <UserTable key={refreshKey} />
    </>
  )
}
