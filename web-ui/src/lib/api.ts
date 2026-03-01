import { authHeaders } from './auth'

const BASE = '/_ui/api'

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers: { ...authHeaders(), ...init?.headers },
  })
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return res.json()
}

export async function apiPut<T>(path: string, body: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

export async function postSetup(data: {
  proxy_api_key: string
  kiro_refresh_token: string
  region: string
}): Promise<void> {
  const res = await fetch(`${BASE}/setup`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }
}

export async function getConfigSchema(): Promise<Record<string, unknown>> {
  return apiFetch<Record<string, unknown>>('/config/schema')
}

export async function checkSetupStatus(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/config`)
    if (!res.ok) return false
    const data = await res.json()
    return data.setup_complete !== false
  } catch {
    return false
  }
}

// --- OAuth ---

export interface OAuthBrowserResponse {
  flow: 'browser'
  authorize_url: string
}

export interface OAuthDeviceResponse {
  flow: 'device'
  user_code: string
  verification_uri: string
  verification_uri_complete: string
  device_code_id: string
}

export type OAuthStartResponse = OAuthBrowserResponse | OAuthDeviceResponse

export async function startOAuth(data: {
  region: string
  proxy_api_key: string
  flow: 'browser' | 'device'
  start_url?: string
}): Promise<OAuthStartResponse> {
  const res = await fetch(`${BASE}/oauth/start`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(data),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }
  return res.json()
}

export interface DevicePollResponse {
  status: 'pending' | 'slow_down' | 'complete'
}

export async function pollDeviceCode(deviceCodeId: string): Promise<DevicePollResponse> {
  const res = await fetch(`${BASE}/oauth/device/poll`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ device_code_id: deviceCodeId }),
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }
  return res.json()
}
