//! Admin API 业务逻辑服务

use std::sync::Arc;

use futures::StreamExt;
use futures::stream::FuturesUnordered;
use tokio::task;
use tracing::warn;

use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;

use super::error::AdminServiceError;
use super::types::{BalanceResponse, CredentialStatusItem, CredentialsStatusResponse};

/// Admin 服务
///
/// 封装所有 Admin API 的业务逻辑
#[derive(Clone)]
pub struct AdminService {
    token_manager: Arc<MultiTokenManager>,
}

impl AdminService {
    pub fn new(token_manager: Arc<MultiTokenManager>) -> Self {
        Self { token_manager }
    }

    /// 获取所有凭据状态（异步获取余额）
    pub async fn get_all_credentials(&self) -> CredentialsStatusResponse {
        let snapshot = self.token_manager.snapshot();

        // 并行获取所有账号的余额
        let balances: Vec<_> = snapshot
            .entries
            .iter()
            .map(|entry| {
                let token_manager = self.token_manager.clone();
                async move {
                    match token_manager.get_usage_limits_for(entry.id).await {
                        Ok(usage) => {
                            let current_usage = usage.current_usage();
                            let usage_limit = usage.usage_limit();
                            let remaining = (usage_limit - current_usage).max(0.0);
                            let usage_percentage = if usage_limit > 0.0 {
                                (current_usage / usage_limit * 100.0).min(100.0)
                            } else {
                                0.0
                            };
                            (
                                entry.id,
                                Some(usage),
                                current_usage,
                                usage_limit,
                                remaining,
                                usage_percentage,
                            )
                        }
                        Err(e) => {
                            warn!("获取凭据 #{} 余额失败: {}", entry.id, e);
                            (entry.id, None, 0.0, 0.0, 0.0, 0.0)
                        }
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_iter()
            .collect::<FuturesUnordered<_>>()
            .collect()
            .await;

        // 构建余额查找表
        let balance_map: std::collections::HashMap<u64, _> = balances
            .into_iter()
            .map(
                |(id, usage, current_usage, usage_limit, remaining, usage_percentage)| {
                    (
                        id,
                        (
                            usage,
                            current_usage,
                            usage_limit,
                            remaining,
                            usage_percentage,
                        ),
                    )
                },
            )
            .collect();

        let credentials: Vec<CredentialStatusItem> = snapshot
            .entries
            .into_iter()
            .map(|entry| {
                let (usage, current_usage, usage_limit, remaining, usage_percentage) = balance_map
                    .get(&entry.id)
                    .cloned()
                    .unwrap_or((None, 0.0, 0.0, 0.0, 0.0));

                CredentialStatusItem {
                    id: entry.id,
                    priority: entry.priority,
                    disabled: entry.disabled,
                    failure_count: entry.failure_count,
                    is_current: entry.id == snapshot.current_id,
                    expires_at: entry.expires_at,
                    auth_method: entry.auth_method,
                    has_profile_arn: entry.has_profile_arn,
                    machine_id: entry.machine_id,
                    subscription_title: usage
                        .as_ref()
                        .and_then(|u| u.subscription_title().map(|s| s.to_string())),
                    current_usage,
                    usage_limit,
                    remaining,
                    usage_percentage,
                    next_reset_at: usage.as_ref().and_then(|u| u.next_date_reset),
                }
            })
            .collect();

        // 异步更新数据库中的余额（不阻塞响应）
        let service = self.clone();
        task::spawn(async move {
            for (id, (usage, _, _, _, _)) in balance_map {
                if let Some(usage) = usage
                    && let Err(e) = service.token_manager.database().update_balance(
                        id,
                        usage.subscription_title(),
                        usage.current_usage(),
                        usage.usage_limit(),
                        usage.next_date_reset,
                    )
                {
                    warn!("异步更新余额到数据库失败 #{}: {}", id, e);
                }
            }
        });

        CredentialsStatusResponse {
            total: snapshot.total,
            available: snapshot.available,
            current_id: snapshot.current_id,
            credentials,
        }
    }

    /// 设置凭据禁用状态
    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<(), AdminServiceError> {
        // 先获取当前凭据 ID，用于判断是否需要切换
        let snapshot = self.token_manager.snapshot();
        let current_id = snapshot.current_id;

        self.token_manager
            .set_disabled(id, disabled)
            .map_err(|e| self.classify_error(e, id))?;

        // 只有禁用的是当前凭据时才尝试切换到下一个
        if disabled && id == current_id {
            let _ = self.token_manager.switch_to_next();
        }
        Ok(())
    }

    /// 设置凭据优先级
    pub fn set_priority(&self, id: u64, priority: u32) -> Result<(), AdminServiceError> {
        self.token_manager
            .set_priority(id, priority)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 重置失败计数并重新启用
    pub fn reset_and_enable(&self, id: u64) -> Result<(), AdminServiceError> {
        self.token_manager
            .reset_and_enable(id)
            .map_err(|e| self.classify_error(e, id))
    }

    /// 获取凭据余额
    pub async fn get_balance(&self, id: u64) -> Result<BalanceResponse, AdminServiceError> {
        let usage = self
            .token_manager
            .get_usage_limits_for(id)
            .await
            .map_err(|e| self.classify_balance_error(e, id))?;

        let current_usage = usage.current_usage();
        let usage_limit = usage.usage_limit();
        let remaining = (usage_limit - current_usage).max(0.0);
        let usage_percentage = if usage_limit > 0.0 {
            (current_usage / usage_limit * 100.0).min(100.0)
        } else {
            0.0
        };

        // 更新余额到数据库
        if let Err(e) = self.token_manager.database().update_balance(
            id,
            usage.subscription_title(),
            current_usage,
            usage_limit,
            usage.next_date_reset,
        ) {
            tracing::warn!("更新余额到数据库失败（不影响本次请求）: {}", e);
        }

        Ok(BalanceResponse {
            id,
            subscription_title: usage.subscription_title().map(|s| s.to_string()),
            current_usage,
            usage_limit,
            remaining,
            usage_percentage,
            next_reset_at: usage.next_date_reset,
        })
    }

    /// 添加新凭据
    ///
    /// 添加后会尝试获取一次余额信息并存储到数据库
    pub async fn add_credential(
        &self,
        refresh_token: String,
        auth_method: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        machine_id: Option<String>,
        priority: Option<u32>,
    ) -> Result<u64, AdminServiceError> {
        // 验证 machine_id 格式（如果提供）
        if let Some(ref mid) = machine_id
            && !crate::kiro::machine_id::is_valid_machine_id(mid)
        {
            return Err(AdminServiceError::InvalidRequest(
                "machineId 必须是有效的 UUID v4 格式（36 字符）".to_string(),
            ));
        }

        // 检查 client_id 是否已存在（去重）
        if let Some(ref cid) = client_id
            && self
                .token_manager
                .database()
                .client_id_exists(cid)
                .map_err(|e| AdminServiceError::InternalError(e.to_string()))?
        {
            return Err(AdminServiceError::InvalidRequest("账号已存在".to_string()));
        }

        let cred = KiroCredentials {
            id: None,
            access_token: None,
            refresh_token: Some(refresh_token),
            profile_arn: None,
            expires_at: None,
            auth_method,
            client_id,
            client_secret,
            machine_id,
            priority: priority.unwrap_or(0),
            disabled: false,
            failure_count: 0,
            subscription_title: None,
            current_usage: 0.0,
            usage_limit: 0.0,
            next_reset_at: None,
            balance_updated_at: None,
        };

        let id = self
            .token_manager
            .add_credential(cred)
            .map_err(|e| AdminServiceError::InternalError(e.to_string()))?;

        // 尝试获取一次余额信息（失败不影响添加结果）
        match self.token_manager.get_usage_limits_for(id).await {
            Ok(usage) => {
                if let Err(e) = self.token_manager.database().update_balance(
                    id,
                    usage.subscription_title(),
                    usage.current_usage(),
                    usage.usage_limit(),
                    usage.next_date_reset,
                ) {
                    tracing::warn!("初始化余额到数据库失败: {}", e);
                } else {
                    tracing::info!("凭据 #{} 初始余额已获取并保存", id);
                }
            }
            Err(e) => {
                tracing::warn!("获取凭据 #{} 初始余额失败（不影响添加结果）: {}", id, e);
            }
        }

        Ok(id)
    }

    /// 删除凭据
    pub fn delete_credential(&self, id: u64) -> Result<(), AdminServiceError> {
        match self.token_manager.delete_credential(id) {
            Ok(true) => Ok(()),
            Ok(false) => Err(AdminServiceError::NotFound { id }),
            Err(e) => Err(AdminServiceError::InternalError(e.to_string())),
        }
    }

    /// 分类简单操作错误（set_disabled, set_priority, reset_and_enable）
    fn classify_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();
        if msg.contains("不存在") {
            AdminServiceError::NotFound { id }
        } else {
            AdminServiceError::InternalError(msg)
        }
    }

    /// 分类余额查询错误（可能涉及上游 API 调用）
    fn classify_balance_error(&self, e: anyhow::Error, id: u64) -> AdminServiceError {
        let msg = e.to_string();

        // 1. 凭据不存在
        if msg.contains("不存在") {
            return AdminServiceError::NotFound { id };
        }

        // 2. 上游服务错误特征：HTTP 响应错误或网络错误
        let is_upstream_error =
            // HTTP 响应错误（来自 refresh_*_token 的错误消息）
            msg.contains("凭证已过期或无效") ||
            msg.contains("权限不足") ||
            msg.contains("已被限流") ||
            msg.contains("服务器错误") ||
            msg.contains("Token 刷新失败") ||
            msg.contains("暂时不可用") ||
            // 网络错误（reqwest 错误）
            msg.contains("error trying to connect") ||
            msg.contains("connection") ||
            msg.contains("timeout") ||
            msg.contains("timed out");

        if is_upstream_error {
            AdminServiceError::UpstreamError(msg)
        } else {
            // 3. 默认归类为内部错误（本地验证失败、配置错误等）
            // 包括：缺少 refreshToken、refreshToken 已被截断、无法生成 machineId 等
            AdminServiceError::InternalError(msg)
        }
    }
}
