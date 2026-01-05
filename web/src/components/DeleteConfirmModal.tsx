import { AlertTriangle, X } from 'lucide-react'

interface DeleteConfirmModalProps {
  isOpen: boolean
  onClose: () => void
  onConfirm: () => void
  accountEmail: string
}

export function DeleteConfirmModal({
  isOpen,
  onClose,
  onConfirm,
  accountEmail,
}: DeleteConfirmModalProps) {
  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={onClose}
      />
      <div className="relative w-full max-w-sm card-elevated p-6 animate-slide-up">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-destructive/10 flex items-center justify-center">
              <AlertTriangle className="w-5 h-5 text-destructive" />
            </div>
            <h2 className="text-lg font-semibold">确认删除</h2>
          </div>
          <button
            onClick={onClose}
            className="btn-ghost p-2 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <p className="text-muted-foreground mb-6">
          确定要删除账号 <span className="font-medium text-foreground">{accountEmail}</span> 吗？此操作无法撤销。
        </p>

        <div className="flex gap-3">
          <button onClick={onClose} className="btn-secondary flex-1">
            取消
          </button>
          <button
            onClick={onConfirm}
            className="btn-primary flex-1 bg-destructive hover:bg-destructive/90"
          >
            删除
          </button>
        </div>
      </div>
    </div>
  )
}
