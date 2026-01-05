// 对齐后端 src/admin/types.rs

/** 单个账号状态 */
export interface Credential {
  id: number
  priority: number
  disabled: boolean
  failureCount: number
  isCurrent: boolean
  expiresAt: string | null
  authMethod: string | null
  hasProfileArn: boolean
  // 余额信息
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  remaining: number
  usagePercentage: number
  nextResetAt: number | null
  machineId: string | null
  email: string | null
}

/** 账号列表响应 */
export interface CredentialsResponse {
  total: number
  available: number
  currentId: number
  credentials: Credential[]
}

/** 添加账号请求 */
export interface AddCredentialRequest {
  refreshToken: string
  authMethod?: string
  clientId?: string
  clientSecret?: string
  machineId?: string // UUID v4 格式，36 字符
  priority?: number
}

/** 添加账号响应 */
export interface AddCredentialResponse {
  success: boolean
  message: string
  id: number
}

/** 设置禁用状态请求 */
export interface SetDisabledRequest {
  disabled: boolean
}

/** 设置优先级请求 */
export interface SetPriorityRequest {
  priority: number
}

/** 余额响应 */
export interface BalanceResponse {
  id: number
  subscriptionTitle: string | null
  currentUsage: number
  usageLimit: number
  remaining: number
  usagePercentage: number
  nextResetAt: number | null
}

/** 通用成功响应 */
export interface SuccessResponse {
  success: boolean
  message: string
}

/** 错误响应 */
export interface ErrorResponse {
  error: {
    type: string
    message: string
  }
}

/** 导入的账号格式（来自 credentials.json） */
export interface ImportCredential {
  id?: number
  accessToken?: string
  refreshToken: string
  expiresAt?: string
  authMethod?: string
  clientId?: string
  clientSecret?: string
  machineId?: string
  profileArn?: string
  priority?: number
}
