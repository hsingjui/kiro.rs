import { getStoredPassword } from '@/components/PasswordSettingModal'
import type {
  CredentialsResponse,
  AddCredentialRequest,
  AddCredentialResponse,
  SetDisabledRequest,
  SetPriorityRequest,
  BalanceResponse,
  SuccessResponse,
  ErrorResponse,
} from '@/types/credential'

const API_BASE = '/api/admin'

class ApiError extends Error {
  type: string
  status: number

  constructor(type: string, message: string, status: number) {
    super(message)
    this.name = 'ApiError'
    this.type = type
    this.status = status
  }
}

async function request<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const apiKey = getStoredPassword()
  if (!apiKey) {
    throw new ApiError('authentication_error', '请先设置 API Key', 401)
  }

  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      'x-api-key': apiKey,
      ...options.headers,
    },
  })

  if (!response.ok) {
    const data = (await response.json()) as ErrorResponse
    throw new ApiError(
      data.error?.type || 'unknown_error',
      data.error?.message || '请求失败',
      response.status
    )
  }

  return response.json() as Promise<T>
}

/** 获取所有账号 */
export async function getCredentials(): Promise<CredentialsResponse> {
  return request<CredentialsResponse>('/credentials')
}

/** 添加账号 */
export async function addCredential(
  data: AddCredentialRequest
): Promise<AddCredentialResponse> {
  return request<AddCredentialResponse>('/credentials', {
    method: 'POST',
    body: JSON.stringify(data),
  })
}

/** 删除账号 */
export async function deleteCredential(id: number): Promise<SuccessResponse> {
  return request<SuccessResponse>(`/credentials/${id}`, {
    method: 'DELETE',
  })
}

/** 设置账号禁用状态 */
export async function setCredentialDisabled(
  id: number,
  disabled: boolean
): Promise<SuccessResponse> {
  return request<SuccessResponse>(`/credentials/${id}/disabled`, {
    method: 'POST',
    body: JSON.stringify({ disabled } as SetDisabledRequest),
  })
}

/** 设置账号优先级 */
export async function setCredentialPriority(
  id: number,
  priority: number
): Promise<SuccessResponse> {
  return request<SuccessResponse>(`/credentials/${id}/priority`, {
    method: 'POST',
    body: JSON.stringify({ priority } as SetPriorityRequest),
  })
}

/** 重置失败计数 */
export async function resetCredentialFailure(
  id: number
): Promise<SuccessResponse> {
  return request<SuccessResponse>(`/credentials/${id}/reset`, {
    method: 'POST',
  })
}

/** 获取账号余额 */
export async function getCredentialBalance(
  id: number
): Promise<BalanceResponse> {
  return request<BalanceResponse>(`/credentials/${id}/balance`)
}

export { ApiError }
