//! Kiro OAuth 凭证数据模型
//!
//! 凭证存储在 SQLite 数据库中

use serde::{Deserialize, Serialize};

/// Kiro OAuth 凭证
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KiroCredentials {
    /// 凭据唯一标识符（自增 ID）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,

    /// 访问令牌
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,

    /// 刷新令牌
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// Profile ARN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_arn: Option<String>,

    /// 过期时间 (RFC3339 格式)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,

    /// 认证方式 (social / idc / builder-id)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<String>,

    /// OIDC Client ID (IdC 认证需要)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,

    /// OIDC Client Secret (IdC 认证需要)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,

    /// 设备指纹（UUID v4 格式）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_id: Option<String>,

    /// 凭据优先级（数字越小优先级越高，默认为 0）
    #[serde(default)]
    #[serde(skip_serializing_if = "is_zero")]
    pub priority: u32,

    // ======== 运行时状态字段（不序列化到 JSON 配置文件）========
    /// 是否禁用
    #[serde(skip)]
    pub disabled: bool,

    /// 连续失败次数
    #[serde(skip)]
    pub failure_count: u32,

    // ======== 余额相关字段（不序列化到 JSON 配置文件）========
    /// 订阅类型
    #[serde(skip)]
    pub subscription_title: Option<String>,

    /// 当前使用量
    #[serde(skip)]
    pub current_usage: f64,

    /// 使用限额
    #[serde(skip)]
    pub usage_limit: f64,

    /// 下次重置时间（Unix 时间戳）
    #[serde(skip)]
    pub next_reset_at: Option<f64>,

    /// 余额更新时间（RFC3339 格式）
    #[serde(skip)]
    pub balance_updated_at: Option<String>,

    /// 账号邮箱（不序列化到 JSON 配置文件）
    #[serde(skip)]
    pub email: Option<String>,
}

/// 判断是否为零（用于跳过序列化）
fn is_zero(value: &u32) -> bool {
    *value == 0
}
