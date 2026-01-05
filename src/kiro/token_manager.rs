//! Token 管理模块
//!
//! 负责 Token 过期检测和刷新，支持 Social 和 IdC 认证方式
//! 支持单凭据 (TokenManager) 和多凭据 (MultiTokenManager) 管理

use anyhow::bail;
use chrono::{DateTime, Duration, Utc};
use parking_lot::Mutex;
use serde::Serialize;
use tokio::sync::Mutex as TokioMutex;

use std::sync::Arc;

use crate::http_client::{ProxyConfig, build_client};
use crate::kiro::db::Database;
use crate::kiro::machine_id;
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::model::token_refresh::{
    IdcRefreshRequest, IdcRefreshResponse, RefreshRequest, RefreshResponse,
};
use crate::kiro::model::usage_limits::UsageLimitsResponse;
use crate::model::config::Config;

/// Token 管理器
///
/// 负责管理凭据和 Token 的自动刷新
#[allow(dead_code)]
pub struct TokenManager {
    config: Config,
    credentials: KiroCredentials,
    proxy: Option<ProxyConfig>,
}

#[allow(dead_code)]
impl TokenManager {
    /// 创建新的 TokenManager 实例
    pub fn new(config: Config, credentials: KiroCredentials, proxy: Option<ProxyConfig>) -> Self {
        Self {
            config,
            credentials,
            proxy,
        }
    }

    /// 获取凭据的引用
    pub fn credentials(&self) -> &KiroCredentials {
        &self.credentials
    }

    /// 获取配置的引用
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// 确保获取有效的访问 Token
    ///
    /// 如果 Token 过期或即将过期，会自动刷新
    pub async fn ensure_valid_token(&mut self) -> anyhow::Result<String> {
        if is_token_expired(&self.credentials) || is_token_expiring_soon(&self.credentials) {
            self.credentials =
                refresh_token(&self.credentials, &self.config, self.proxy.as_ref()).await?;

            // 刷新后再次检查 token 时间有效性
            if is_token_expired(&self.credentials) {
                anyhow::bail!("刷新后的 Token 仍然无效或已过期");
            }
        }

        self.credentials
            .access_token
            .clone()
            .ok_or_else(|| anyhow::anyhow!("没有可用的 accessToken"))
    }

    /// 获取使用额度信息
    ///
    /// 调用 getUsageLimits API 查询当前账户的使用额度
    pub async fn get_usage_limits(&mut self) -> anyhow::Result<UsageLimitsResponse> {
        let token = self.ensure_valid_token().await?;
        get_usage_limits(&self.credentials, &self.config, &token, self.proxy.as_ref()).await
    }
}

/// 检查 Token 是否在指定时间内过期
pub(crate) fn is_token_expiring_within(
    credentials: &KiroCredentials,
    minutes: i64,
) -> Option<bool> {
    credentials
        .expires_at
        .as_ref()
        .and_then(|expires_at| DateTime::parse_from_rfc3339(expires_at).ok())
        .map(|expires| expires <= Utc::now() + Duration::minutes(minutes))
}

/// 检查 Token 是否已过期（提前 5 分钟判断）
pub(crate) fn is_token_expired(credentials: &KiroCredentials) -> bool {
    is_token_expiring_within(credentials, 5).unwrap_or(true)
}

/// 检查 Token 是否即将过期（10分钟内）
pub(crate) fn is_token_expiring_soon(credentials: &KiroCredentials) -> bool {
    is_token_expiring_within(credentials, 10).unwrap_or(false)
}

/// 验证 refreshToken 的基本有效性
pub(crate) fn validate_refresh_token(credentials: &KiroCredentials) -> anyhow::Result<()> {
    let refresh_token = credentials
        .refresh_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("缺少 refreshToken"))?;

    if refresh_token.is_empty() {
        bail!("refreshToken 为空");
    }

    if refresh_token.len() < 100 || refresh_token.ends_with("...") || refresh_token.contains("...")
    {
        bail!(
            "refreshToken 已被截断（长度: {} 字符）。\n\
             这通常是 Kiro IDE 为了防止凭证被第三方工具使用而故意截断的。",
            refresh_token.len()
        );
    }

    Ok(())
}

/// 刷新 Token
pub(crate) async fn refresh_token(
    credentials: &KiroCredentials,
    config: &Config,
    proxy: Option<&ProxyConfig>,
) -> anyhow::Result<KiroCredentials> {
    validate_refresh_token(credentials)?;

    // 根据 auth_method 选择刷新方式
    let auth_method = credentials.auth_method.as_deref().unwrap_or("social");

    match auth_method.to_lowercase().as_str() {
        "idc" | "builder-id" => refresh_idc_token(credentials, config, proxy).await,
        _ => refresh_social_token(credentials, config, proxy).await,
    }
}

/// 刷新 Social Token
async fn refresh_social_token(
    credentials: &KiroCredentials,
    config: &Config,
    proxy: Option<&ProxyConfig>,
) -> anyhow::Result<KiroCredentials> {
    tracing::info!("正在刷新 Social Token...");

    let refresh_token = credentials.refresh_token.as_ref().unwrap();
    let region = &config.region;

    let refresh_url = format!("https://prod.{}.auth.desktop.kiro.dev/refreshToken", region);
    let refresh_domain = format!("prod.{}.auth.desktop.kiro.dev", region);
    let machine_id = machine_id::generate_from_credentials(credentials)
        .ok_or_else(|| anyhow::anyhow!("无法生成 machineId"))?;
    let kiro_version = &config.kiro_version;

    let client = build_client(proxy, 60)?;
    let body = RefreshRequest {
        refresh_token: refresh_token.to_string(),
    };

    let response = client
        .post(&refresh_url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Content-Type", "application/json")
        .header(
            "User-Agent",
            format!("KiroIDE-{}-{}", kiro_version, machine_id),
        )
        .header("Accept-Encoding", "gzip, compress, deflate, br")
        .header("host", &refresh_domain)
        .header("Connection", "close")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let error_msg = match status.as_u16() {
            401 => "OAuth 凭证已过期或无效，需要重新认证",
            403 => "权限不足，无法刷新 Token",
            429 => "请求过于频繁，已被限流",
            500..=599 => "服务器错误，AWS OAuth 服务暂时不可用",
            _ => "Token 刷新失败",
        };
        bail!("{}: {} {}", error_msg, status, body_text);
    }

    let data: RefreshResponse = response.json().await?;

    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(data.access_token);

    if let Some(new_refresh_token) = data.refresh_token {
        new_credentials.refresh_token = Some(new_refresh_token);
    }

    if let Some(profile_arn) = data.profile_arn {
        new_credentials.profile_arn = Some(profile_arn);
    }

    if let Some(expires_in) = data.expires_in {
        let expires_at = Utc::now() + Duration::seconds(expires_in);
        new_credentials.expires_at = Some(expires_at.to_rfc3339());
    }

    Ok(new_credentials)
}

/// IdC Token 刷新所需的 x-amz-user-agent header
const IDC_AMZ_USER_AGENT: &str = "aws-sdk-js/3.738.0 ua/2.1 os/other lang/js md/browser#unknown_unknown api/sso-oidc#3.738.0 m/E KiroIDE";

/// 刷新 IdC Token (AWS SSO OIDC)
async fn refresh_idc_token(
    credentials: &KiroCredentials,
    config: &Config,
    proxy: Option<&ProxyConfig>,
) -> anyhow::Result<KiroCredentials> {
    tracing::info!("正在刷新 IdC Token...");

    let refresh_token = credentials.refresh_token.as_ref().unwrap();
    let client_id = credentials
        .client_id
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("IdC 刷新需要 clientId"))?;
    let client_secret = credentials
        .client_secret
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("IdC 刷新需要 clientSecret"))?;

    let region = &config.region;
    let refresh_url = format!("https://oidc.{}.amazonaws.com/token", region);

    let client = build_client(proxy, 60)?;
    let body = IdcRefreshRequest {
        client_id: client_id.to_string(),
        client_secret: client_secret.to_string(),
        refresh_token: refresh_token.to_string(),
        grant_type: "refresh_token".to_string(),
    };

    let response = client
        .post(&refresh_url)
        .header("Content-Type", "application/json")
        .header("Host", format!("oidc.{}.amazonaws.com", region))
        .header("Connection", "keep-alive")
        .header("x-amz-user-agent", IDC_AMZ_USER_AGENT)
        .header("Accept", "*/*")
        .header("Accept-Language", "*")
        .header("sec-fetch-mode", "cors")
        .header("User-Agent", "node")
        .header("Accept-Encoding", "br, gzip, deflate")
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let error_msg = match status.as_u16() {
            401 => "IdC 凭证已过期或无效，需要重新认证",
            403 => "权限不足，无法刷新 Token",
            429 => "请求过于频繁，已被限流",
            500..=599 => "服务器错误，AWS OIDC 服务暂时不可用",
            _ => "IdC Token 刷新失败",
        };
        bail!("{}: {} {}", error_msg, status, body_text);
    }

    let data: IdcRefreshResponse = response.json().await?;

    let mut new_credentials = credentials.clone();
    new_credentials.access_token = Some(data.access_token);

    if let Some(new_refresh_token) = data.refresh_token {
        new_credentials.refresh_token = Some(new_refresh_token);
    }

    if let Some(expires_in) = data.expires_in {
        let expires_at = Utc::now() + Duration::seconds(expires_in);
        new_credentials.expires_at = Some(expires_at.to_rfc3339());
    }

    Ok(new_credentials)
}

/// getUsageLimits API 所需的 x-amz-user-agent header 前缀
const USAGE_LIMITS_AMZ_USER_AGENT_PREFIX: &str = "aws-sdk-js/1.0.0";

/// 获取使用额度信息
pub(crate) async fn get_usage_limits(
    credentials: &KiroCredentials,
    config: &Config,
    token: &str,
    proxy: Option<&ProxyConfig>,
) -> anyhow::Result<UsageLimitsResponse> {
    tracing::debug!("正在获取使用额度信息...");

    let region = &config.region;
    let host = format!("q.{}.amazonaws.com", region);
    let machine_id = machine_id::generate_from_credentials(credentials)
        .ok_or_else(|| anyhow::anyhow!("无法生成 machineId"))?;
    let kiro_version = &config.kiro_version;

    // 构建 URL
    let mut url = format!(
        "https://{}/getUsageLimits?origin=AI_EDITOR&resourceType=AGENTIC_REQUEST",
        host
    );

    // profileArn 是可选的
    if let Some(profile_arn) = &credentials.profile_arn {
        url.push_str(&format!("&profileArn={}", urlencoding::encode(profile_arn)));
    }

    // 构建 User-Agent headers
    let user_agent = format!(
        "aws-sdk-js/1.0.0 ua/2.1 os/darwin#24.6.0 lang/js md/nodejs#22.21.1 \
         api/codewhispererruntime#1.0.0 m/N,E KiroIDE-{}-{}",
        kiro_version, machine_id
    );
    let amz_user_agent = format!(
        "{} KiroIDE-{}-{}",
        USAGE_LIMITS_AMZ_USER_AGENT_PREFIX, kiro_version, machine_id
    );

    let client = build_client(proxy, 60)?;

    let response = client
        .get(&url)
        .header("x-amz-user-agent", &amz_user_agent)
        .header("User-Agent", &user_agent)
        .header("host", &host)
        .header("amz-sdk-invocation-id", uuid::Uuid::new_v4().to_string())
        .header("amz-sdk-request", "attempt=1; max=1")
        .header("Authorization", format!("Bearer {}", token))
        .header("Connection", "close")
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await.unwrap_or_default();
        let error_msg = match status.as_u16() {
            401 => "认证失败，Token 无效或已过期",
            403 => "权限不足，无法获取使用额度",
            429 => "请求过于频繁，已被限流",
            500..=599 => "服务器错误，AWS 服务暂时不可用",
            _ => "获取使用额度失败",
        };
        bail!("{}: {} {}", error_msg, status, body_text);
    }

    let data: UsageLimitsResponse = response.json().await?;
    Ok(data)
}

// ============================================================================
// 多凭据 Token 管理器
// ============================================================================

// ============================================================================
// Admin API 公开结构
// ============================================================================

/// 凭据条目快照（用于 Admin API 读取）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialEntrySnapshot {
    /// 凭据唯一 ID
    pub id: u64,
    /// 优先级
    pub priority: u32,
    /// 是否被禁用
    pub disabled: bool,
    /// 连续失败次数
    pub failure_count: u32,
    /// 认证方式
    pub auth_method: Option<String>,
    /// 是否有 Profile ARN
    pub has_profile_arn: bool,
    /// Token 过期时间
    pub expires_at: Option<String>,
    /// 设备指纹（UUID v4 格式）
    pub machine_id: Option<String>,
}

/// 凭据管理器状态快照
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagerSnapshot {
    /// 凭据条目列表
    pub entries: Vec<CredentialEntrySnapshot>,
    /// 当前活跃凭据 ID
    pub current_id: u64,
    /// 总凭据数量
    pub total: usize,
    /// 可用凭据数量
    pub available: usize,
}

/// 多凭据 Token 管理器
///
/// 支持多个凭据的管理，实现固定优先级 + 故障转移策略
/// 故障统计基于 API 调用结果，而非 Token 刷新结果
///
/// 所有凭据状态（包括 disabled、failure_count）完全存储在 SQLite 中，
/// 不维护内存缓存。
pub struct MultiTokenManager {
    config: Config,
    proxy: Option<ProxyConfig>,
    /// 当前活动凭据 ID（仅内存，重启后按优先级重新选择）
    current_id: Mutex<u64>,
    /// Token 刷新锁，确保同一时间只有一个刷新操作
    refresh_lock: TokioMutex<()>,
    /// SQLite 数据库连接（唯一数据源）
    db: Arc<Database>,
}

/// 每个凭据最大 API 调用失败次数
const MAX_FAILURES_PER_CREDENTIAL: u32 = 3;

/// 禁用凭据自动恢复冷却时间（秒）
const DISABLED_COOLDOWN_SECONDS: i64 = 300; // 5 分钟

/// API 调用上下文
///
/// 绑定特定凭据的调用上下文，确保 token、credentials 和 id 的一致性
/// 用于解决并发调用时 current_id 竞态问题
#[derive(Clone)]
pub struct CallContext {
    /// 凭据 ID（用于 report_success/report_failure）
    pub id: u64,
    /// 凭据信息（用于构建请求头）
    pub credentials: KiroCredentials,
    /// 访问 Token
    pub token: String,
}

impl MultiTokenManager {
    /// 创建多凭据 Token 管理器
    ///
    /// 从 SQLite 数据库读取优先级最高的可用凭据作为初始凭据
    ///
    /// # Arguments
    /// * `config` - 应用配置
    /// * `db` - 数据库连接
    /// * `proxy` - 可选的代理配置
    pub fn new(
        config: Config,
        db: Arc<Database>,
        proxy: Option<ProxyConfig>,
    ) -> anyhow::Result<Self> {
        // 选择初始凭据：优先级最高（priority 最小）的可用凭据
        let initial_id = db
            .get_highest_priority_available()?
            .and_then(|c| c.id)
            .unwrap_or(0);

        Ok(Self {
            config,
            proxy,
            current_id: Mutex::new(initial_id),
            refresh_lock: TokioMutex::new(()),
            db,
        })
    }

    /// 获取数据库引用
    pub fn database(&self) -> &Arc<Database> {
        &self.db
    }

    /// 获取配置的引用
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// 获取当前活动凭据的克隆
    pub fn credentials(&self) -> KiroCredentials {
        let current_id = *self.current_id.lock();
        self.db
            .get_credential(current_id)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    /// 获取凭据总数
    pub fn total_count(&self) -> usize {
        self.db.count_credentials().unwrap_or(0)
    }

    /// 获取可用凭据数量
    pub fn available_count(&self) -> usize {
        self.db.count_available().unwrap_or(0)
    }

    /// 获取 API 调用上下文
    ///
    /// 返回绑定了 id、credentials 和 token 的调用上下文
    /// 确保整个 API 调用过程中使用一致的凭据信息
    ///
    /// 如果 Token 过期或即将过期，会自动刷新
    /// Token 刷新失败时会尝试下一个可用凭据（不计入失败次数）
    ///
    /// 会自动恢复冷却期已过的禁用凭据
    pub async fn acquire_context(&self) -> anyhow::Result<CallContext> {
        // 尝试恢复冷却期已过的禁用凭据
        if let Err(e) = self.db.try_recover_disabled(DISABLED_COOLDOWN_SECONDS) {
            tracing::warn!("尝试恢复禁用凭据失败: {}", e);
        }

        let total = self.total_count();
        let mut tried_count = 0;

        loop {
            if tried_count >= total {
                anyhow::bail!(
                    "所有凭据均无法获取有效 Token（可用: {}/{}）",
                    self.available_count(),
                    total
                );
            }

            let (id, credentials) = {
                let current_id = *self.current_id.lock();

                // 尝试获取当前凭据
                if let Some(cred) = self.db.get_credential(current_id)? {
                    if !cred.disabled {
                        (current_id, cred)
                    } else {
                        // 当前凭据已禁用，选择优先级最高的可用凭据
                        if let Some(cred) = self.db.get_highest_priority_available()? {
                            let new_id = cred.id.unwrap();
                            *self.current_id.lock() = new_id;
                            (new_id, cred)
                        } else {
                            anyhow::bail!(
                                "所有凭据均已禁用（{}/{}）",
                                self.available_count(),
                                total
                            );
                        }
                    }
                } else {
                    // 当前凭据不存在，选择优先级最高的可用凭据
                    if let Some(cred) = self.db.get_highest_priority_available()? {
                        let new_id = cred.id.unwrap();
                        *self.current_id.lock() = new_id;
                        (new_id, cred)
                    } else {
                        anyhow::bail!("所有凭据均已禁用（{}/{}）", self.available_count(), total);
                    }
                }
            };

            // 尝试获取/刷新 Token
            match self.try_ensure_token(id, &credentials).await {
                Ok(ctx) => {
                    return Ok(ctx);
                }
                Err(e) => {
                    tracing::warn!("凭据 #{} Token 刷新失败，尝试下一个凭据: {}", id, e);

                    // Token 刷新失败，切换到下一个优先级的凭据（不计入失败次数）
                    self.switch_to_next_by_priority();
                    tried_count += 1;
                }
            }
        }
    }

    /// 切换到下一个优先级最高的可用凭据（内部方法）
    fn switch_to_next_by_priority(&self) {
        let current_id = *self.current_id.lock();

        // 选择优先级最高的未禁用凭据（排除当前凭据）
        if let Ok(Some(cred)) = self.db.get_next_available(current_id) {
            let new_id = cred.id.unwrap();
            *self.current_id.lock() = new_id;
            tracing::info!("已切换到凭据 #{}（优先级 {}）", new_id, cred.priority);
        }
    }

    /// 选择优先级最高的未禁用凭据作为当前凭据（内部方法）
    ///
    /// 与 `switch_to_next_by_priority` 不同，此方法不排除当前凭据，
    /// 纯粹按优先级选择，用于优先级变更后立即生效
    fn select_highest_priority(&self) {
        let current_id = *self.current_id.lock();

        // 选择优先级最高的未禁用凭据（不排除当前凭据）
        if let Ok(Some(best)) = self.db.get_highest_priority_available() {
            let best_id = best.id.unwrap();
            if best_id != current_id {
                tracing::info!(
                    "优先级变更后切换凭据: #{} -> #{}（优先级 {}）",
                    current_id,
                    best_id,
                    best.priority
                );
                *self.current_id.lock() = best_id;
            }
        }
    }

    /// 尝试使用指定凭据获取有效 Token
    ///
    /// 使用双重检查锁定模式，确保同一时间只有一个刷新操作
    ///
    /// # Arguments
    /// * `id` - 凭据 ID，用于更新正确的条目
    /// * `credentials` - 凭据信息
    async fn try_ensure_token(
        &self,
        id: u64,
        credentials: &KiroCredentials,
    ) -> anyhow::Result<CallContext> {
        // 第一次检查（无锁）：快速判断是否需要刷新
        let needs_refresh = is_token_expired(credentials) || is_token_expiring_soon(credentials);

        let creds = if needs_refresh {
            // 获取刷新锁，确保同一时间只有一个刷新操作
            let _guard = self.refresh_lock.lock().await;

            // 第二次检查：获取锁后重新读取凭据，因为其他请求可能已经完成刷新
            let current_creds = self
                .db
                .get_credential(id)?
                .ok_or_else(|| anyhow::anyhow!("凭据 #{} 不存在", id))?;

            if is_token_expired(&current_creds) || is_token_expiring_soon(&current_creds) {
                // 确实需要刷新
                let new_creds =
                    refresh_token(&current_creds, &self.config, self.proxy.as_ref()).await?;

                if is_token_expired(&new_creds) {
                    anyhow::bail!("刷新后的 Token 仍然无效或已过期");
                }

                // 回写凭据到数据库
                self.db.update_credential(&new_creds)?;
                tracing::debug!("已持久化凭据 #{} 到数据库", id);

                new_creds
            } else {
                // 其他请求已经完成刷新，直接使用新凭据
                tracing::debug!("Token 已被其他请求刷新，跳过刷新");
                current_creds
            }
        } else {
            credentials.clone()
        };

        let token = creds
            .access_token
            .clone()
            .ok_or_else(|| anyhow::anyhow!("没有可用的 accessToken"))?;

        Ok(CallContext {
            id,
            credentials: creds,
            token,
        })
    }

    /// 报告指定凭据 API 调用成功
    ///
    /// 重置该凭据的失败计数（持久化到数据库）
    ///
    /// # Arguments
    /// * `id` - 凭据 ID（来自 CallContext）
    pub fn report_success(&self, id: u64) {
        if let Err(e) = self.db.reset_failure_count(id) {
            tracing::warn!("重置凭据 #{} 失败计数失败: {}", id, e);
        } else {
            tracing::debug!("凭据 #{} API 调用成功", id);
        }
    }

    /// 报告指定凭据 API 调用失败
    ///
    /// 增加失败计数，达到阈值时禁用凭据并切换到优先级最高的可用凭据
    /// 返回是否还有可用凭据可以重试
    ///
    /// # Arguments
    /// * `id` - 凭据 ID（来自 CallContext）
    pub fn report_failure(&self, id: u64) -> bool {
        // 增加失败计数
        let failure_count = match self.db.increment_failure_count(id) {
            Ok(count) => count,
            Err(e) => {
                tracing::warn!("增加凭据 #{} 失败计数失败: {}", id, e);
                return self.available_count() > 0;
            }
        };

        tracing::warn!(
            "凭据 #{} API 调用失败（{}/{}）",
            id,
            failure_count,
            MAX_FAILURES_PER_CREDENTIAL
        );

        if failure_count >= MAX_FAILURES_PER_CREDENTIAL {
            // 禁用凭据
            if let Err(e) = self.db.set_disabled(id, true) {
                tracing::warn!("禁用凭据 #{} 失败: {}", id, e);
            }
            tracing::error!("凭据 #{} 已连续失败 {} 次，已被禁用", id, failure_count);

            // 切换到优先级最高的可用凭据
            if let Ok(Some(next)) = self.db.get_highest_priority_available() {
                let next_id = next.id.unwrap();
                *self.current_id.lock() = next_id;
                tracing::info!("已切换到凭据 #{}（优先级 {}）", next_id, next.priority);
            } else {
                tracing::error!("所有凭据均已禁用！");
                return false;
            }
        }

        // 检查是否还有可用凭据
        self.available_count() > 0
    }

    /// 切换到优先级最高的可用凭据
    ///
    /// 返回是否成功切换
    pub fn switch_to_next(&self) -> bool {
        let current_id = *self.current_id.lock();

        // 选择优先级最高的未禁用凭据（排除当前凭据）
        if let Ok(Some(next)) = self.db.get_next_available(current_id) {
            let next_id = next.id.unwrap();
            *self.current_id.lock() = next_id;
            tracing::info!("已切换到凭据 #{}（优先级 {}）", next_id, next.priority);
            true
        } else {
            // 没有其他可用凭据，检查当前凭据是否可用
            self.db
                .get_credential(current_id)
                .ok()
                .flatten()
                .map(|c| !c.disabled)
                .unwrap_or(false)
        }
    }

    /// 获取使用额度信息
    #[allow(dead_code)]
    pub async fn get_usage_limits(&self) -> anyhow::Result<UsageLimitsResponse> {
        let ctx = self.acquire_context().await?;
        get_usage_limits(
            &ctx.credentials,
            &self.config,
            &ctx.token,
            self.proxy.as_ref(),
        )
        .await
    }

    // ========================================================================
    // Admin API 方法
    // ========================================================================

    /// 获取管理器状态快照（用于 Admin API）
    pub fn snapshot(&self) -> ManagerSnapshot {
        let credentials = self.db.load_credentials().unwrap_or_default();
        let current_id = *self.current_id.lock();
        let available = credentials.iter().filter(|c| !c.disabled).count();

        ManagerSnapshot {
            entries: credentials
                .iter()
                .map(|c| CredentialEntrySnapshot {
                    id: c.id.unwrap_or(0),
                    priority: c.priority,
                    disabled: c.disabled,
                    failure_count: c.failure_count,
                    auth_method: c.auth_method.clone(),
                    has_profile_arn: c.profile_arn.is_some(),
                    expires_at: c.expires_at.clone(),
                    machine_id: c.machine_id.clone(),
                })
                .collect(),
            current_id,
            total: credentials.len(),
            available,
        }
    }

    /// 设置凭据禁用状态（Admin API）
    ///
    /// 持久化到数据库
    pub fn set_disabled(&self, id: u64, disabled: bool) -> anyhow::Result<()> {
        if !disabled {
            // 启用时重置失败计数
            self.db.reset_and_enable(id)?;
        } else {
            self.db.set_disabled(id, true)?;
        }
        Ok(())
    }

    /// 设置凭据优先级（Admin API）
    ///
    /// 修改优先级后会立即按新优先级重新选择当前凭据。
    pub fn set_priority(&self, id: u64, priority: u32) -> anyhow::Result<()> {
        // 持久化更改到数据库
        self.db.set_priority(id, priority)?;
        // 立即按新优先级重新选择当前凭据
        self.select_highest_priority();
        Ok(())
    }

    /// 重置凭据失败计数并重新启用（Admin API）
    ///
    /// 持久化到数据库
    pub fn reset_and_enable(&self, id: u64) -> anyhow::Result<()> {
        self.db.reset_and_enable(id)?;
        Ok(())
    }

    /// 添加新凭据（Admin API）
    ///
    /// 写入数据库，返回新凭据的 ID
    pub fn add_credential(&self, cred: KiroCredentials) -> anyhow::Result<u64> {
        // 写入数据库
        let id = self.db.insert_credential(&cred)?;

        // 如果这是第一个凭据，设置为当前凭据
        if self.total_count() == 1 {
            *self.current_id.lock() = id;
        }

        tracing::info!("已添加新凭据 #{}", id);
        Ok(id)
    }

    /// 删除凭据（Admin API）
    ///
    /// 从数据库中删除，如果删除的是当前凭据会自动切换
    pub fn delete_credential(&self, id: u64) -> anyhow::Result<bool> {
        let current_id = *self.current_id.lock();
        let need_switch = id == current_id;

        // 从数据库删除
        let deleted = self.db.delete_credential(id)?;
        if !deleted {
            return Ok(false);
        }

        // 如果删除的是当前凭据，切换到下一个
        if need_switch {
            self.select_highest_priority();
        }

        tracing::info!("已删除凭据 #{}", id);
        Ok(true)
    }

    /// 获取指定凭据的使用额度（Admin API）
    pub async fn get_usage_limits_for(&self, id: u64) -> anyhow::Result<UsageLimitsResponse> {
        let credentials = self
            .db
            .get_credential(id)?
            .ok_or_else(|| anyhow::anyhow!("凭据不存在: {}", id))?;

        // 检查是否需要刷新 token
        let needs_refresh = is_token_expired(&credentials) || is_token_expiring_soon(&credentials);

        let (token, final_creds) = if needs_refresh {
            let _guard = self.refresh_lock.lock().await;
            let current_creds = self
                .db
                .get_credential(id)?
                .ok_or_else(|| anyhow::anyhow!("凭据不存在: {}", id))?;

            if is_token_expired(&current_creds) || is_token_expiring_soon(&current_creds) {
                let new_creds =
                    refresh_token(&current_creds, &self.config, self.proxy.as_ref()).await?;
                // 持久化到数据库
                self.db.update_credential(&new_creds)?;
                let token = new_creds
                    .access_token
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("刷新后无 access_token"))?;
                (token, new_creds)
            } else {
                let token = current_creds
                    .access_token
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("凭据无 access_token"))?;
                (token, current_creds)
            }
        } else {
            let token = credentials
                .access_token
                .clone()
                .ok_or_else(|| anyhow::anyhow!("凭据无 access_token"))?;
            (token, credentials)
        };

        get_usage_limits(&final_creds, &self.config, &token, self.proxy.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_manager_new() {
        let config = Config::default();
        let credentials = KiroCredentials::default();
        let tm = TokenManager::new(config, credentials, None);
        assert!(tm.credentials().access_token.is_none());
    }

    #[test]
    fn test_is_token_expired_with_expired_token() {
        let mut credentials = KiroCredentials::default();
        credentials.expires_at = Some("2020-01-01T00:00:00Z".to_string());
        assert!(is_token_expired(&credentials));
    }

    #[test]
    fn test_is_token_expired_with_valid_token() {
        let mut credentials = KiroCredentials::default();
        let future = Utc::now() + Duration::hours(1);
        credentials.expires_at = Some(future.to_rfc3339());
        assert!(!is_token_expired(&credentials));
    }

    #[test]
    fn test_is_token_expired_within_5_minutes() {
        let mut credentials = KiroCredentials::default();
        let expires = Utc::now() + Duration::minutes(3);
        credentials.expires_at = Some(expires.to_rfc3339());
        assert!(is_token_expired(&credentials));
    }

    #[test]
    fn test_is_token_expired_no_expires_at() {
        let credentials = KiroCredentials::default();
        assert!(is_token_expired(&credentials));
    }

    #[test]
    fn test_is_token_expiring_soon_within_10_minutes() {
        let mut credentials = KiroCredentials::default();
        let expires = Utc::now() + Duration::minutes(8);
        credentials.expires_at = Some(expires.to_rfc3339());
        assert!(is_token_expiring_soon(&credentials));
    }

    #[test]
    fn test_is_token_expiring_soon_beyond_10_minutes() {
        let mut credentials = KiroCredentials::default();
        let expires = Utc::now() + Duration::minutes(15);
        credentials.expires_at = Some(expires.to_rfc3339());
        assert!(!is_token_expiring_soon(&credentials));
    }

    #[test]
    fn test_validate_refresh_token_missing() {
        let credentials = KiroCredentials::default();
        let result = validate_refresh_token(&credentials);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_refresh_token_valid() {
        let mut credentials = KiroCredentials::default();
        credentials.refresh_token = Some("a".repeat(150));
        let result = validate_refresh_token(&credentials);
        assert!(result.is_ok());
    }

    // MultiTokenManager 测试

    fn setup_test_db(credentials: Vec<KiroCredentials>) -> Arc<Database> {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        for cred in credentials {
            db.insert_credential(&cred).unwrap();
        }
        // 保持 tempdir 存活
        std::mem::forget(dir);
        db
    }

    #[test]
    fn test_multi_token_manager_new() {
        let config = Config::default();
        let mut cred1 = KiroCredentials::default();
        cred1.refresh_token = Some("token1".to_string());
        cred1.priority = 0;
        let mut cred2 = KiroCredentials::default();
        cred2.refresh_token = Some("token2".to_string());
        cred2.priority = 1;

        let db = setup_test_db(vec![cred1, cred2]);
        let manager = MultiTokenManager::new(config, db, None).unwrap();
        assert_eq!(manager.total_count(), 2);
        assert_eq!(manager.available_count(), 2);
    }

    #[test]
    fn test_multi_token_manager_empty_credentials() {
        let config = Config::default();
        let db = setup_test_db(vec![]);
        // 空凭据现在可以创建成功，但调用 API 时会失败
        let manager = MultiTokenManager::new(config, db, None).unwrap();
        assert_eq!(manager.total_count(), 0);
        assert_eq!(manager.available_count(), 0);
    }

    #[test]
    fn test_multi_token_manager_report_failure() {
        let config = Config::default();
        let mut cred1 = KiroCredentials::default();
        cred1.refresh_token = Some("token1".to_string());
        let mut cred2 = KiroCredentials::default();
        cred2.refresh_token = Some("token2".to_string());

        let db = setup_test_db(vec![cred1, cred2]);
        let manager = MultiTokenManager::new(config, db, None).unwrap();

        // 凭据 ID 由数据库自动分配（从 1 开始）
        // 前两次失败不会禁用（使用 ID 1）
        assert!(manager.report_failure(1));
        assert!(manager.report_failure(1));
        assert_eq!(manager.available_count(), 2);

        // 第三次失败会禁用第一个凭据
        assert!(manager.report_failure(1));
        assert_eq!(manager.available_count(), 1);

        // 继续失败第二个凭据（使用 ID 2）
        assert!(manager.report_failure(2));
        assert!(manager.report_failure(2));
        assert!(!manager.report_failure(2)); // 所有凭据都禁用了
        assert_eq!(manager.available_count(), 0);
    }

    #[test]
    fn test_multi_token_manager_report_success() {
        let config = Config::default();
        let mut cred = KiroCredentials::default();
        cred.refresh_token = Some("token".to_string());

        let db = setup_test_db(vec![cred]);
        let manager = MultiTokenManager::new(config, db, None).unwrap();

        // 失败两次（使用 ID 1）
        manager.report_failure(1);
        manager.report_failure(1);

        // 成功后重置计数（使用 ID 1）
        manager.report_success(1);

        // 再失败两次不会禁用
        manager.report_failure(1);
        manager.report_failure(1);
        assert_eq!(manager.available_count(), 1);
    }

    #[test]
    fn test_multi_token_manager_switch_to_next() {
        let config = Config::default();
        let mut cred1 = KiroCredentials::default();
        cred1.refresh_token = Some("token1".to_string());
        cred1.priority = 0;
        let mut cred2 = KiroCredentials::default();
        cred2.refresh_token = Some("token2".to_string());
        cred2.priority = 1;

        let db = setup_test_db(vec![cred1, cred2]);
        let manager = MultiTokenManager::new(config, db, None).unwrap();

        // 初始是第一个凭据
        assert_eq!(
            manager.credentials().refresh_token,
            Some("token1".to_string())
        );

        // 切换到下一个
        assert!(manager.switch_to_next());
        assert_eq!(
            manager.credentials().refresh_token,
            Some("token2".to_string())
        );
    }
}
