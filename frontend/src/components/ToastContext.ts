import { createContext } from 'react'

type ToastType = 'success' | 'error'

export interface ToastContextValue {
  showToast: (message: string, type?: ToastType) => void
}

export const ToastContext = createContext<ToastContextValue>({ showToast: () => {} })
