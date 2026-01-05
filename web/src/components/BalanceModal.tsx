import { X, TrendingUp } from 'lucide-react'
import type { BalanceResponse } from '@/types/credential'

interface BalanceModalProps {
  isOpen: boolean
  onClose: () => void
  balance: BalanceResponse | null
  loading: boolean
}

export function BalanceModal({ isOpen, onClose, balance, loading }: BalanceModalProps) {
  if (!isOpen) return null

  const formatDate = (timestamp: number | null) => {
    if (!timestamp) return '-'
    return new Date(timestamp * 1000).toLocaleDateString('zh-CN', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
  }

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
              <TrendingUp className="w-5 h-5 text-primary" />
            </div>
            <h2 className="text-lg font-semibold">
              账号余额 #{balance?.id ?? '-'}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="btn-ghost p-2 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {loading ? (
          <div className="py-8 text-center text-muted-foreground">
            <div className="inline-block w-6 h-6 border-2 border-primary border-t-transparent rounded-full animate-spin mb-2" />
            <p className="text-sm">加载中...</p>
          </div>
        ) : balance ? (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-4">
              <div className="p-3 rounded-lg bg-muted/50">
                <p className="text-xs text-muted-foreground mb-1">订阅类型</p>
                <p className="font-medium truncate">{balance.subscriptionTitle || '-'}</p>
              </div>
              <div className="p-3 rounded-lg bg-muted/50">
                <p className="text-xs text-muted-foreground mb-1">使用百分比</p>
                <p className="font-medium font-mono">{balance.usagePercentage.toFixed(1)}%</p>
              </div>
            </div>

            <div className="p-4 rounded-lg bg-muted/50">
              <div className="flex justify-between text-sm mb-2">
                <span className="text-muted-foreground">已使用</span>
                <span className="font-mono">{balance.currentUsage.toFixed(2)}</span>
              </div>
              <div className="w-full h-2 bg-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-primary rounded-full transition-all duration-300"
                  style={{ width: `${Math.min(balance.usagePercentage, 100)}%` }}
                />
              </div>
              <div className="flex justify-between text-sm mt-2">
                <span className="text-muted-foreground">限额</span>
                <span className="font-mono">{balance.usageLimit.toFixed(2)}</span>
              </div>
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div className="p-3 rounded-lg bg-ai-success/10 border border-ai-success/20">
                <p className="text-xs text-muted-foreground mb-1">剩余额度</p>
                <p className="font-medium font-mono text-ai-success">{balance.remaining.toFixed(2)}</p>
              </div>
              <div className="p-3 rounded-lg bg-muted/50">
                <p className="text-xs text-muted-foreground mb-1">下次重置</p>
                <p className="font-medium text-sm">{formatDate(balance.nextResetAt)}</p>
              </div>
            </div>
          </div>
        ) : (
          <div className="py-8 text-center text-muted-foreground">
            无法获取余额信息
          </div>
        )}

        <div className="mt-6">
          <button onClick={onClose} className="btn-secondary w-full">
            关闭
          </button>
        </div>
      </div>
    </div>
  )
}
