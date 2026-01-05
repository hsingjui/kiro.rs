import { useState, useEffect, useCallback } from 'react'
import {
  Trash2,
  Key,
  Settings,
  AlertCircle,
  TrendingUp,
  Pencil,
  RefreshCw,
  Upload,
  CheckCircle,
  Wallet,
} from 'lucide-react'
import type { Credential, BalanceResponse, AddCredentialRequest } from '@/types/credential'
import {
  getCredentials,
  addCredential,
  deleteCredential,
  setCredentialDisabled,
  setCredentialPriority,
  getCredentialBalance,
  ApiError,
} from '@/api/credentials'
import { DeleteConfirmModal } from './DeleteConfirmModal'
import { PasswordSettingModal, getStoredPassword } from './PasswordSettingModal'
import { BalanceModal } from './BalanceModal'
import { PriorityModal } from './PriorityModal'
import { ImportModal } from './ImportModal'

export function Dashboard() {
  const [credentials, setCredentials] = useState<Credential[]>([])
  const [total, setTotal] = useState(0)
  const [available, setAvailable] = useState(0)
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)

  const [isDeleteModalOpen, setIsDeleteModalOpen] = useState(false)
  const [isPasswordModalOpen, setIsPasswordModalOpen] = useState(false)
  const [isBalanceModalOpen, setIsBalanceModalOpen] = useState(false)
  const [isPriorityModalOpen, setIsPriorityModalOpen] = useState(false)
  const [isImportModalOpen, setIsImportModalOpen] = useState(false)

  const [deletingCredential, setDeletingCredential] = useState<Credential | null>(null)
  const [balanceData, setBalanceData] = useState<BalanceResponse | null>(null)
  const [balanceLoading, setBalanceLoading] = useState(false)
  const [editingCredential, setEditingCredential] = useState<Credential | null>(null)

  const [showPasswordWarning, setShowPasswordWarning] = useState(false)
  const [actionLoading, setActionLoading] = useState<number | null>(null)

  const fetchCredentials = useCallback(async () => {
    const apiKey = getStoredPassword()
    if (!apiKey) {
      setShowPasswordWarning(true)
      setLoading(false)
      return
    }

    setShowPasswordWarning(false)
    setLoading(true)
    setError(null)

    try {
      const response = await getCredentials()
      setCredentials(response.credentials)
      setTotal(response.total)
      setAvailable(response.available)
    } catch (e) {
      if (e instanceof ApiError) {
        setError(e.message)
      } else {
        setError('获取账号列表失败')
      }
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    fetchCredentials()
  }, [fetchCredentials])

  const handleDelete = (credential: Credential) => {
    setDeletingCredential(credential)
    setIsDeleteModalOpen(true)
  }

  const handleConfirmDelete = async () => {
    if (!deletingCredential) return

    try {
      await deleteCredential(deletingCredential.id)
      setIsDeleteModalOpen(false)
      setDeletingCredential(null)
      fetchCredentials()
    } catch (e) {
      if (e instanceof ApiError) {
        alert(e.message)
      }
    }
  }

  const handleToggleDisabled = async (credential: Credential) => {
    setActionLoading(credential.id)
    try {
      await setCredentialDisabled(credential.id, !credential.disabled)
      fetchCredentials()
    } catch (e) {
      if (e instanceof ApiError) {
        alert(e.message)
      }
    } finally {
      setActionLoading(null)
    }
  }

  const handleEditPriority = (credential: Credential) => {
    setEditingCredential(credential)
    setIsPriorityModalOpen(true)
  }

  const handleSavePriority = async (priority: number) => {
    if (!editingCredential) return

    try {
      await setCredentialPriority(editingCredential.id, priority)
      setIsPriorityModalOpen(false)
      setEditingCredential(null)
      fetchCredentials()
    } catch (e) {
      if (e instanceof ApiError) {
        alert(e.message)
      }
    }
  }

  const handleShowBalance = async (credential: Credential) => {
    setBalanceData(null)
    setBalanceLoading(true)
    setIsBalanceModalOpen(true)

    try {
      const data = await getCredentialBalance(credential.id)
      setBalanceData(data)
    } catch (e) {
      if (e instanceof ApiError) {
        alert(e.message)
        setIsBalanceModalOpen(false)
      }
    } finally {
      setBalanceLoading(false)
    }
  }

  const handlePasswordSaved = () => {
    setShowPasswordWarning(false)
    fetchCredentials()
  }

  const handleImport = async (
    credentialsList: AddCredentialRequest[]
  ): Promise<{ success: number; failed: number; errors: string[] }> => {
    let success = 0
    let failed = 0
    const errors: string[] = []

    for (let i = 0; i < credentialsList.length; i++) {
      const cred = credentialsList[i]
      try {
        await addCredential(cred)
        success++
      } catch (e) {
        failed++
        const msg = e instanceof ApiError ? e.message : '未知错误'
        errors.push(`#${i + 1}: ${msg}`)
      }
    }

    if (success > 0) {
      fetchCredentials()
    }

    return { success, failed, errors }
  }

  const handleOpenImport = () => {
    if (!getStoredPassword()) {
      setShowPasswordWarning(true)
      return
    }
    setIsImportModalOpen(true)
  }

  const formatDate = (dateStr: string | null) => {
    if (!dateStr) return '-'
    return new Date(dateStr).toLocaleDateString('zh-CN', {
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    })
  }

  return (
    <>
      <header className="mb-8 flex flex-col md:flex-row md:items-center justify-between gap-4 animate-fade-in">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-primary/10 rounded-xl flex items-center justify-center border border-primary/20">
            <Key className="w-5 h-5 text-primary" strokeWidth={1.5} />
          </div>
          <div>
            <h1 className="text-2xl font-semibold tracking-tight">Kiro 账号管理</h1>
            <p className="text-sm text-muted-foreground">Manage your credentials and API keys</p>
          </div>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={fetchCredentials}
            className="btn-ghost text-muted-foreground hover:text-foreground"
            title="刷新"
            disabled={loading}
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
          </button>
          <button
            onClick={() => setIsPasswordModalOpen(true)}
            className="btn-secondary"
            title="设置 API Key"
          >
            <Settings className="w-4 h-4" />
            <span className="hidden sm:inline">API Key</span>
          </button>
        </div>
      </header>

      {showPasswordWarning && (
        <div className="mb-6">
          <div className="flex items-center gap-3 p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-600 dark:text-amber-400 animate-fade-in">
            <AlertCircle className="w-5 h-5 flex-shrink-0" />
            <p className="text-sm">
              请先设置 API Key 以访问账号管理功能。点击右上角「API Key」进行设置。
            </p>
          </div>
        </div>
      )}

      {error && (
        <div className="mb-6">
          <div className="flex items-center gap-3 p-4 rounded-xl bg-destructive/10 border border-destructive/20 text-destructive animate-fade-in">
            <AlertCircle className="w-5 h-5 flex-shrink-0" />
            <p className="text-sm">{error}</p>
          </div>
        </div>
      )}

      <div className="grid grid-cols-2 sm:grid-cols-3 gap-4 mb-8 animate-slide-up">
        <div className="group relative overflow-hidden rounded-2xl border border-border/50 bg-background/50 p-5 backdrop-blur-xl transition-all duration-300 hover:shadow-md hover:bg-background/60">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-xs font-medium text-muted-foreground mb-1">总账号数</p>
              <p className="text-2xl font-bold font-mono tabular-nums tracking-tight text-foreground">{total}</p>
            </div>
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary/10 text-primary transition-transform duration-300 group-hover:scale-110">
              <Key className="w-5 h-5" strokeWidth={1.5} />
            </div>
          </div>
        </div>
        <div className="group relative overflow-hidden rounded-2xl border border-emerald-500/20 bg-emerald-500/5 p-5 backdrop-blur-xl transition-all duration-300 hover:shadow-md hover:bg-emerald-500/10">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-xs font-medium text-emerald-600/80 dark:text-emerald-400/80 mb-1">可用账号</p>
              <p className="text-2xl font-bold font-mono tabular-nums tracking-tight text-emerald-600 dark:text-emerald-400">
                {available}
              </p>
            </div>
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 transition-transform duration-300 group-hover:scale-110">
              <CheckCircle className="w-5 h-5" strokeWidth={1.5} />
            </div>
          </div>
        </div>
        <div className="group relative overflow-hidden rounded-2xl border border-blue-500/20 bg-blue-500/5 p-5 backdrop-blur-xl transition-all duration-300 hover:shadow-md hover:bg-blue-500/10 col-span-2 sm:col-span-1">
          <div className="flex items-center justify-between gap-3">
            <div>
              <p className="text-xs font-medium text-blue-600/80 dark:text-blue-400/80 mb-1">总可用额度</p>
              <p className="text-2xl font-bold font-mono tabular-nums tracking-tight text-blue-600 dark:text-blue-400">
                ${credentials.reduce((sum, c) => sum + (c.remaining || 0), 0).toFixed(2)}
              </p>
            </div>
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-blue-500/20 text-blue-600 dark:text-blue-400 transition-transform duration-300 group-hover:scale-110">
              <Wallet className="w-5 h-5" strokeWidth={1.5} />
            </div>
          </div>
        </div>
      </div>

      <div className="card overflow-hidden animate-slide-up" style={{ animationDelay: '0.1s' }}>
        <div className="p-4 md:p-6 border-b border-border flex items-center justify-between">
          <h2 className="font-semibold text-lg">账号列表</h2>
          <div className="flex items-center gap-2">
            <button onClick={handleOpenImport} className="btn-primary">
              <Upload className="w-4 h-4" />
              导入账号
            </button>
          </div>
        </div>

        {loading ? (
          <div className="py-24 text-center">
            <div className="inline-block w-8 h-8 border-2 border-primary border-t-transparent rounded-full animate-spin mb-3" />
            <p className="text-muted-foreground">加载中...</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="table-header">
                  <th className="text-left px-4 md:px-6 py-4 whitespace-nowrap">ID</th>
                  <th className="text-left px-4 md:px-6 py-4 whitespace-nowrap">优先级</th>
                  <th className="text-left px-4 md:px-6 py-4 whitespace-nowrap">状态</th>
                  <th className="text-left px-4 md:px-6 py-4 hidden md:table-cell whitespace-nowrap">额度</th>
                  <th className="text-left px-4 md:px-6 py-4 hidden md:table-cell whitespace-nowrap">失败次数</th>
                  <th className="text-left px-4 md:px-6 py-4 hidden lg:table-cell whitespace-nowrap">过期时间</th>
                  <th className="text-left px-4 md:px-6 py-4 hidden lg:table-cell whitespace-nowrap">认证方式</th>
                  <th className="text-left px-4 md:px-6 py-4 hidden lg:table-cell whitespace-nowrap">机器码</th>
                  <th className="text-right px-4 md:px-6 py-4 whitespace-nowrap">操作</th>
                </tr>
              </thead>
              <tbody>
                {credentials.length === 0 ? (
                  <tr>
                    <td colSpan={10} className="px-6 py-24">
                      <div className="empty-state">
                        <div className="w-16 h-16 bg-secondary rounded-full flex items-center justify-center mb-4">
                          <Key className="w-8 h-8 text-muted-foreground" />
                        </div>
                        <p className="font-medium text-lg text-foreground">暂无账号</p>
                        <p className="text-sm mt-1 text-muted-foreground">点击右上角"导入账号"添加第一个账号</p>
                      </div>
                    </td>
                  </tr>
                ) : (
                  credentials.map((credential) => (
                    <tr key={credential.id} className="table-row-hover group">
                      <td className="table-cell px-4 md:px-6 py-4 whitespace-nowrap">
                        <div className="flex items-center gap-2">
                          <span className="font-mono font-medium text-sm">#{credential.id}</span>
                        </div>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 whitespace-nowrap">
                        <button
                          onClick={() => handleEditPriority(credential)}
                          className="inline-flex items-center gap-1 px-2 py-1 rounded hover:bg-muted/50 transition-colors group/btn"
                          title="点击修改优先级"
                        >
                          <span className="font-mono text-sm">{credential.priority}</span>
                          <Pencil className="w-3 h-3 text-muted-foreground group-hover/btn:text-foreground transition-colors" />
                        </button>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 whitespace-nowrap">
                        <button
                          onClick={() => handleToggleDisabled(credential)}
                          disabled={actionLoading === credential.id}
                          className="inline-flex items-center gap-1.5 transition-opacity hover:opacity-80"
                          title={credential.disabled ? '点击启用' : '点击禁用'}
                        >
                          {credential.disabled ? (
                            <span className="badge-default">禁用</span>
                          ) : (
                            <span className="badge-success">启用</span>
                          )}
                        </button>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 hidden md:table-cell whitespace-nowrap">
                        {credential.usageLimit > 0 ? (
                          <div className="space-y-1.5 min-w-[120px]">
                            <div className="flex items-baseline justify-between gap-2">
                              <span className="font-mono tabular-nums text-xs font-medium text-muted-foreground">
                                {credential.currentUsage.toFixed(1)} / {credential.usageLimit.toFixed(1)}
                              </span>
                              <span className={`text-[10px] font-mono tabular-nums ${
                                credential.usagePercentage >= 90 ? 'text-destructive' : 'text-muted-foreground'
                              }`}>
                                {credential.usagePercentage.toFixed(1)}%
                              </span>
                            </div>
                            <div className="w-full h-1.5 bg-secondary rounded-full overflow-hidden">
                              <div
                                className={`h-full rounded-full transition-all duration-500 ${
                                  credential.usagePercentage >= 90
                                    ? 'bg-destructive'
                                    : credential.usagePercentage >= 70
                                      ? 'bg-amber-500'
                                      : 'bg-ai-success'
                                }`}
                                style={{ width: `${Math.min(credential.usagePercentage, 100)}%` }}
                              />
                            </div>
                          </div>
                        ) : (
                          <span className="text-muted-foreground text-sm">-</span>
                        )}
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 hidden md:table-cell whitespace-nowrap">
                        <span
                          className={`font-mono text-sm ${
                            credential.failureCount > 0 ? 'text-destructive font-medium' : 'text-muted-foreground'
                          }`}
                        >
                          {credential.failureCount}
                        </span>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 hidden lg:table-cell text-muted-foreground text-sm font-mono tabular-nums whitespace-nowrap">
                        {formatDate(credential.expiresAt)}
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 hidden lg:table-cell whitespace-nowrap">
                        <span className="text-xs px-2 py-1 rounded bg-secondary text-secondary-foreground border border-border">
                          {credential.authMethod || '-'}
                        </span>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 hidden lg:table-cell whitespace-nowrap overflow-hidden">
                        <span className="text-xs font-mono tabular-nums text-muted-foreground max-w-[120px] inline-block truncate" title={credential.machineId || ''}>
                          {credential.machineId || '-'}
                        </span>
                      </td>
                      <td className="table-cell px-4 md:px-6 py-4 whitespace-nowrap">
                        <div className="flex items-center justify-end gap-1">
                          <button
                            onClick={() => handleShowBalance(credential)}
                            className="btn-ghost p-2 h-8 w-8 text-muted-foreground hover:text-primary"
                            title="查看余额"
                          >
                            <TrendingUp className="w-4 h-4" />
                          </button>
                          <button
                            onClick={() => handleDelete(credential)}
                            className="btn-ghost p-2 h-8 w-8 text-muted-foreground hover:text-destructive"
                            title="删除"
                          >
                            <Trash2 className="w-4 h-4" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <DeleteConfirmModal
        isOpen={isDeleteModalOpen}
        onClose={() => {
          setIsDeleteModalOpen(false)
          setDeletingCredential(null)
        }}
        onConfirm={handleConfirmDelete}
        accountEmail={`账号 #${deletingCredential?.id ?? ''}`}
      />

      <PasswordSettingModal
        isOpen={isPasswordModalOpen}
        onClose={() => setIsPasswordModalOpen(false)}
        onSave={handlePasswordSaved}
      />

      <BalanceModal
        isOpen={isBalanceModalOpen}
        onClose={() => setIsBalanceModalOpen(false)}
        balance={balanceData}
        loading={balanceLoading}
      />

      <PriorityModal
        isOpen={isPriorityModalOpen}
        onClose={() => {
          setIsPriorityModalOpen(false)
          setEditingCredential(null)
        }}
        onSave={handleSavePriority}
        credentialId={editingCredential?.id ?? null}
        currentPriority={editingCredential?.priority ?? 0}
      />

      <ImportModal
        isOpen={isImportModalOpen}
        onClose={() => setIsImportModalOpen(false)}
        onImport={handleImport}
      />
    </>
  )
}
