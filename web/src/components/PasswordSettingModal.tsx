import { useState, useEffect, type FormEvent } from 'react'
import { X, Eye, EyeOff, Key } from 'lucide-react'

const STORAGE_KEY = 'kiro_admin_password'

interface PasswordSettingModalProps {
  isOpen: boolean
  onClose: () => void
  onSave: () => void
}

export function getStoredPassword(): string | null {
  return localStorage.getItem(STORAGE_KEY)
}

export function setStoredPassword(password: string): void {
  localStorage.setItem(STORAGE_KEY, password)
}

export function PasswordSettingModal({ isOpen, onClose, onSave }: PasswordSettingModalProps) {
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [error, setError] = useState('')

  const hasExistingPassword = !!getStoredPassword()

  useEffect(() => {
    if (isOpen) {
      setPassword('')
      setError('')
    }
  }, [isOpen])

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    setError('')

    if (!password.trim()) {
      setError('请输入密码')
      return
    }

    if (password.length < 4) {
      setError('密码至少需要4个字符')
      return
    }

    setStoredPassword(password)
    onSave()
    onClose()
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative w-full max-w-sm card-elevated p-6 animate-slide-up">
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
              <Key className="w-5 h-5 text-primary" />
            </div>
            <h2 className="text-lg font-semibold">
              {hasExistingPassword ? '修改密码' : '设置密码'}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="btn-ghost p-2 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="new-password" className="block text-sm font-medium mb-2">
              {hasExistingPassword ? '新密码' : '密码'}
            </label>
            <div className="relative">
              <input
                id="new-password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="输入密码"
                className="input pr-10"
                autoFocus
              />
              <button
                type="button"
                onClick={() => setShowPassword(!showPassword)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors duration-200"
              >
                {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>
          </div>

          {error && (
            <p className="text-sm text-destructive animate-fade-in">{error}</p>
          )}

          <div className="flex gap-3 pt-2">
            <button type="button" onClick={onClose} className="btn-secondary flex-1">
              取消
            </button>
            <button type="submit" className="btn-primary flex-1">
              保存
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
