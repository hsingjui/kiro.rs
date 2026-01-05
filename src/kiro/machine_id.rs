//! 设备指纹生成器
//!

use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::kiro::model::credentials::KiroCredentials;

/// 验证 machine_id 格式是否有效（UUID v4）
///
/// UUID v4 格式: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx (36字符)
/// 例如: b3981d12-4d61-418c-9b77-461db82a7cc4
pub fn is_valid_machine_id(machine_id: &str) -> bool {
    // UUID v4 格式: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx (36字符)
    if machine_id.len() != 36 {
        return false;
    }

    // 检查连字符位置
    let parts: Vec<&str> = machine_id.split('-').collect();
    if parts.len() != 5 {
        return false;
    }

    // 检查每段长度和格式
    if parts[0].len() != 8
        || parts[1].len() != 4
        || parts[2].len() != 4
        || parts[3].len() != 4
        || parts[4].len() != 12
    {
        return false;
    }

    // 检查是否都是十六进制字符
    for part in parts {
        if !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }

    true
}

/// 根据凭证信息生成唯一的 Machine ID
///
/// 优先使用凭据的 machine_id，然后使用 profileArn 生成，否则使用 refreshToken 生成
pub fn generate_from_credentials(credentials: &KiroCredentials) -> Option<String> {
    // 如果凭据配置了 machineId 且为有效 UUID v4，优先使用
    if let Some(ref machine_id) = credentials.machine_id
        && is_valid_machine_id(machine_id)
    {
        return Some(machine_id.clone());
    }

    // 如果有有效的 profileArn 则使用 profileArn 固定指纹
    if let Some(ref profile_arn) = credentials.profile_arn
        && is_valid_profile_arn(profile_arn)
    {
        return Some(generate_uuid_from_seed(&format!(
            "KotlinNativeAPI/{}",
            profile_arn
        )));
    }

    // 使用 refreshToken 生成
    if let Some(ref refresh_token) = credentials.refresh_token
        && !refresh_token.is_empty()
    {
        return Some(generate_uuid_from_seed(&format!(
            "KotlinNativeAPI/{}",
            refresh_token
        )));
    }

    // 没有有效的凭证
    None
}

/// 验证 profileArn 是否有效
fn is_valid_profile_arn(profile_arn: &str) -> bool {
    !profile_arn.is_empty()
        && profile_arn.starts_with("arn:aws")
        && profile_arn.contains("profile/")
}

/// 从种子生成确定性的 UUID v4
pub fn generate_uuid_from_seed(seed: &str) -> String {
    // 使用 SHA256 哈希种子，然后转换为 UUID v4 格式
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let result = hasher.finalize();

    // 取前 16 字节构造 UUID
    let uuid = Uuid::from_bytes(result[..16].try_into().unwrap());

    // 转换为 UUID v4 格式字符串
    uuid.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_machine_id() {
        // 有效的 UUID v4
        assert!(is_valid_machine_id("b3981d12-4d61-418c-9b77-461db82a7cc4"));

        // 无效的长度
        assert!(!is_valid_machine_id("b3981d12"));
        assert!(!is_valid_machine_id(
            "b3981d12-4d61-418c-9b77-461db82a7cc4-extra"
        ));

        // 无效的格式（缺少连字符）
        assert!(!is_valid_machine_id("b3981d124d61418c9b77461db82a7cc4"));

        // 无效的字符
        assert!(!is_valid_machine_id("b3981d12-4d61-418c-9b7x-461db82a7cc4"));

        // 空字符串
        assert!(!is_valid_machine_id(""));
    }

    #[test]
    fn test_is_valid_profile_arn() {
        assert!(is_valid_profile_arn("arn:aws:sso::123456789:profile/test"));
        assert!(!is_valid_profile_arn("invalid"));
        assert!(!is_valid_profile_arn("arn:aws:sso::123456789"));
        assert!(!is_valid_profile_arn(""));
    }

    #[test]
    fn test_generate_uuid_from_seed() {
        let result = generate_uuid_from_seed("test");
        assert_eq!(result.len(), 36);
        assert!(result.contains('-'));
        // 验证是有效的 UUID 格式
        assert!(is_valid_machine_id(&result));

        // 确定性：相同种子应生成相同 UUID
        let result2 = generate_uuid_from_seed("test");
        assert_eq!(result, result2);

        // 不同种子生成不同 UUID
        let result3 = generate_uuid_from_seed("test2");
        assert_ne!(result, result3);
    }

    #[test]
    fn test_generate_with_credential_machine_id() {
        let mut credentials = KiroCredentials::default();
        credentials.machine_id = Some("b3981d12-4d61-418c-9b77-461db82a7cc4".to_string());

        let result = generate_from_credentials(&credentials);
        assert_eq!(
            result,
            Some("b3981d12-4d61-418c-9b77-461db82a7cc4".to_string())
        );
    }

    #[test]
    fn test_generate_with_invalid_credential_machine_id() {
        let mut credentials = KiroCredentials::default();
        // 旧的 64 字符格式现在被视为无效
        credentials.machine_id = Some("a".repeat(64));
        credentials.profile_arn = Some("arn:aws:sso::123456789:profile/test".to_string());

        let result = generate_from_credentials(&credentials);
        // 应该回退到使用 profileArn 生成
        assert!(result.is_some());
        assert!(is_valid_machine_id(result.as_ref().unwrap()));
        assert_eq!(result.as_ref().unwrap().len(), 36);
    }

    #[test]
    fn test_generate_with_profile_arn() {
        let mut credentials = KiroCredentials::default();
        credentials.profile_arn = Some("arn:aws:sso::123456789:profile/test".to_string());

        let result = generate_from_credentials(&credentials);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().len(), 36);
        assert!(is_valid_machine_id(result.as_ref().unwrap()));
    }

    #[test]
    fn test_generate_with_refresh_token() {
        let mut credentials = KiroCredentials::default();
        credentials.refresh_token = Some("test_refresh_token".to_string());

        let result = generate_from_credentials(&credentials);
        assert!(result.is_some());
        assert_eq!(result.as_ref().unwrap().len(), 36);
        assert!(is_valid_machine_id(result.as_ref().unwrap()));
    }

    #[test]
    fn test_generate_without_credentials() {
        let credentials = KiroCredentials::default();

        let result = generate_from_credentials(&credentials);
        assert!(result.is_none());
    }

    #[test]
    fn test_credential_machine_id_priority() {
        // 凭据的 machine_id 应该优先于 profileArn
        let mut credentials = KiroCredentials::default();
        credentials.profile_arn = Some("arn:aws:sso::123456789:profile/test".to_string());
        credentials.machine_id = Some("b3981d12-4d61-418c-9b77-461db82a7cc4".to_string());

        let result = generate_from_credentials(&credentials);
        assert_eq!(
            result,
            Some("b3981d12-4d61-418c-9b77-461db82a7cc4".to_string())
        );
    }
}
