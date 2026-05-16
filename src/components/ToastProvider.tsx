import React, { createContext, useContext, useState, useCallback, useRef, useEffect } from 'react'

interface Toast {
  id: string
  message: string
  type: 'success' | 'error' | 'info'
}

interface ToastContextType {
  addToast: (message: string, type: Toast['type']) => string
  removeToast: (id: string) => void
}

const ToastContext = createContext<ToastContextType | null>(null)

export function useToast(): ToastContextType {
  const context = useContext(ToastContext)
  if (!context) {
    throw new Error('useToast must be used within a ToastProvider')
  }
  return context
}

const TOAST_ICONS: Record<Toast['type'], string> = {
  success: '✓',
  error: '✗',
  info: 'ℹ',
}

const AUTO_DISMISS_DELAY = 4000

interface ToastProviderProps {
  children: React.ReactNode
}

export default function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<Toast[]>([])
  const timeoutsRef = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map())

  const removeToast = useCallback((id: string) => {
    setToasts((prev) => prev.filter((t) => t.id !== id))
    const timeout = timeoutsRef.current.get(id)
    if (timeout) {
      clearTimeout(timeout)
      timeoutsRef.current.delete(id)
    }
  }, [])

  const addToast = useCallback((message: string, type: Toast['type']): string => {
    const id = `toast-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
    const toast: Toast = { id, message, type }

    setToasts((prev) => [...prev, toast])

    if (type === 'success' || type === 'info') {
      const timeout = setTimeout(() => {
        removeToast(id)
      }, AUTO_DISMISS_DELAY)
      timeoutsRef.current.set(id, timeout)
    }

    return id
  }, [removeToast])

  useEffect(() => {
    return () => {
      timeoutsRef.current.forEach((timeout) => clearTimeout(timeout))
    }
  }, [])

  return (
    <ToastContext.Provider value={{ addToast, removeToast }}>
      {children}
      <div className="toast-container" role="region" aria-label="Notifications">
        {toasts.map((toast) => (
          <div
            key={toast.id}
            className={`toast toast-${toast.type}`}
            role={toast.type === 'error' ? 'alert' : 'status'}
            aria-live={toast.type === 'error' ? 'assertive' : 'polite'}
          >
            <span className="toast-icon" aria-hidden="true">
              {TOAST_ICONS[toast.type]}
            </span>
            <span className="toast-message">{toast.message}</span>
            <button
              className="toast-close"
              onClick={() => removeToast(toast.id)}
              aria-label="Dismiss notification"
              type="button"
            >
              ×
            </button>
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}