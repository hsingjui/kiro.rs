import { useState, type FormEvent } from 'react'
import { X, Upload, CheckCircle, XCircle } from 'lucide-react'
import type { ImportCredential, AddCredentialRequest } from '@/types/credential'

interface ImportModalProps {
  isOpen: boolean
  onClose: () => void
  onImport: (credentials: AddCredentialRequest[]) => Promise<{ success: number; failed: number; errors: string[] }>
}

export function ImportModal({ isOpen, onClose, onImport }: ImportModalProps) {
  const [jsonText, setJsonText] = useState('')
  const [error, setError] = useState('')
  const [importing, setImporting] = useState(false)
  const [result, setResult] = useState<{ success: number; failed: number; errors: string[] } | null>(null)

  const parseCredentials = (text: string): AddCredentialRequest[] => {
    const json = JSON.parse(text)
    const items: ImportCredential[] = Array.isArray(json) ? json : [json]

    return items
      .filter((item) => item.refreshToken)
      .map((item) => ({
        refreshToken: item.refreshToken,
        authMethod: item.authMethod,
        clientId: item.clientId,
        clientSecret: item.clientSecret,
        machineId: item.machineId,
        priority: item.priority ?? 0,
      }))
  }

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    setResult(null)

    if (!jsonText.trim()) {
      setError('请输入 JSON 数据')
      return
    }

    let credentials: AddCredentialRequest[]
    try {
      credentials = parseCredentials(jsonText)
    } catch {
      setError('JSON 格式无效')
      return
    }

    if (credentials.length === 0) {
      setError('没有找到有效的账号（需要包含 refreshToken 字段）')
      return
    }

    setImporting(true)
    try {
      const importResult = await onImport(credentials)
      setResult(importResult)
      if (importResult.success > 0 && importResult.failed === 0) {
        setTimeout(() => {
          handleClose()
        }, 1500)
      }
    } catch {
      setError('导入过程中发生错误')
    } finally {
      setImporting(false)
    }
  }

  const handleClose = () => {
    setJsonText('')
    setError('')
    setResult(null)
    onClose()
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div
        className="absolute inset-0 bg-background/80 backdrop-blur-sm"
        onClick={handleClose}
      />
      <div className="relative w-full max-w-lg card-elevated p-6 animate-slide-up max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-6">
          <div className="flex items-center gap-3">
            <div className="w-10 h-10 rounded-full bg-primary/10 flex items-center justify-center">
              <Upload className="w-5 h-5 text-primary" />
            </div>
            <h2 className="text-lg font-semibold">批量导入账号</h2>
          </div>
          <button
            onClick={handleClose}
            className="btn-ghost p-2 text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="json-input" className="block text-sm font-medium mb-2">
              JSON 数据
            </label>
            <textarea
              id="json-input"
              value={jsonText}
              onChange={(e) => setJsonText(e.target.value)}
              placeholder={`[
  {
    "refreshToken": "aorAAAAA...",
    "authMethod": "idc",
    "clientId": "xxx",
    "clientSecret": "xxx",
    "machineId": "b3981d12-4d61-418c-9b77-461db82a7cc4"
  }
]`}
              className="input min-h-[200px] font-mono text-sm resize-y"
              autoFocus
              disabled={importing}
            />
            <p className="text-xs text-muted-foreground mt-2">
              支持 credentials.json 格式，必须包含 refreshToken 字段。其他字段（accessToken、expiresAt 等）会被忽略。
            </p>
          </div>

          {error && (
            <div className="flex items-center gap-2 p-3 rounded-lg bg-destructive/10 text-destructive text-sm animate-fade-in">
              <XCircle className="w-4 h-4 flex-shrink-0" />
              {error}
            </div>
          )}

          {result && (
            <div className="space-y-2 animate-fade-in">
              <div className="flex items-center gap-2 p-3 rounded-lg bg-ai-success/10 text-ai-success text-sm">
                <CheckCircle className="w-4 h-4 flex-shrink-0" />
                成功导入 {result.success} 个账号
              </div>
              {result.failed > 0 && (
                <div className="p-3 rounded-lg bg-destructive/10 text-destructive text-sm">
                  <p className="mb-2">失败 {result.failed} 个：</p>
                  <ul className="list-disc list-inside space-y-1">
                    {result.errors.slice(0, 5).map((err, i) => (
                      <li key={i} className="text-xs">{err}</li>
                    ))}
                    {result.errors.length > 5 && (
                      <li className="text-xs">...还有 {result.errors.length - 5} 个错误</li>
                    )}
                  </ul>
                </div>
              )}
            </div>
          )}

          <div className="flex gap-3 pt-2">
            <button type="button" onClick={handleClose} className="btn-secondary flex-1" disabled={importing}>
              取消
            </button>
            <button type="submit" className="btn-primary flex-1" disabled={importing}>
              {importing ? (
                <>
                  <div className="w-4 h-4 border-2 border-primary-foreground border-t-transparent rounded-full animate-spin" />
                  导入中...
                </>
              ) : (
                '导入'
              )}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
