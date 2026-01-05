import { useState, useEffect, type FormEvent } from 'react'
import { X } from 'lucide-react'

interface PriorityModalProps {
  isOpen: boolean
  onClose: () => void
  onSave: (priority: number) => void
  credentialId: number | null
  currentPriority: number
}

export function PriorityModal({
  isOpen,
  onClose,
  onSave,
  credentialId,
  currentPriority,
}: PriorityModalProps) {
  const [priority, setPriority] = useState(0)
  const [error, setError] = useState('')

  useEffect(() => {
    if (isOpen) {
      setPriority(currentPriority)
      setError('')
    }
  }, [isOpen, currentPriority])

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault()
    setError('')

    if (priority < 0) {
      setError('优先级不能为负数')
      return
    }

    onSave(priority)
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative w-full max-w-xs card-elevated p-6 animate-slide-up">
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-lg font-semibold">
            修改优先级 #{credentialId}
          </h2>
          <button
            onClick={onClose}
            className="btn-ghost p-2 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="priority" className="block text-sm font-medium mb-2">
              优先级
            </label>
            <input
              id="priority"
              type="number"
              min="0"
              value={priority}
              onChange={(e) => setPriority(parseInt(e.target.value) || 0)}
              className="input w-full"
              autoFocus
            />
            <p className="text-xs text-muted-foreground mt-1">
              数字越小优先级越高
            </p>
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
