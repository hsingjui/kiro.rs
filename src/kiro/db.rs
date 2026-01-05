//! SQLite 数据库模块
//!
//! 提供凭据的持久化存储

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;

use crate::kiro::model::credentials::KiroCredentials;

/// 数据库连接包装器
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 打开或创建数据库
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Arc<Self>> {
        let path = path.as_ref();

        // 确保父目录存在
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建数据库目录失败: {:?}", parent))?;
        }

        let conn = Connection::open(path).with_context(|| format!("打开数据库失败: {:?}", path))?;

        // 使用 DELETE 模式，只保留单个 db 文件
        conn.execute_batch("PRAGMA journal_mode=DELETE;")?;

        let db = Self {
            conn: Mutex::new(conn),
        };

        db.init_schema()?;

        Ok(Arc::new(db))
    }

    /// 初始化数据库 schema
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS credentials (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                refresh_token TEXT NOT NULL,
                access_token TEXT,
                expires_at TEXT,
                auth_method TEXT DEFAULT 'social',
                client_id TEXT,
                client_secret TEXT,
                profile_arn TEXT,
                priority INTEGER DEFAULT 0,
                disabled INTEGER DEFAULT 0,
                failure_count INTEGER DEFAULT 0,
                disabled_at TEXT,
                subscription_title TEXT,
                current_usage REAL DEFAULT 0,
                usage_limit REAL DEFAULT 0,
                next_reset_at REAL,
                balance_updated_at TEXT,
                machine_id TEXT,
                created_at TEXT DEFAULT CURRENT_TIMESTAMP,
                updated_at TEXT DEFAULT CURRENT_TIMESTAMP
            );

            CREATE INDEX IF NOT EXISTS idx_credentials_priority ON credentials(priority);
            CREATE INDEX IF NOT EXISTS idx_credentials_disabled ON credentials(disabled);
            "#,
        )?;

        Ok(())
    }

    /// 加载所有凭据（按优先级排序）
    pub fn load_credentials(&self) -> Result<Vec<KiroCredentials>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, refresh_token, access_token, expires_at, auth_method,
                   client_id, client_secret, profile_arn, priority,
                   disabled, failure_count,
                   subscription_title, current_usage, usage_limit, next_reset_at, balance_updated_at,
                   machine_id
            FROM credentials
            ORDER BY priority ASC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(KiroCredentials {
                id: Some(row.get::<_, i64>(0)? as u64),
                refresh_token: row.get(1)?,
                access_token: row.get(2)?,
                expires_at: row.get(3)?,
                auth_method: row.get(4)?,
                client_id: row.get(5)?,
                client_secret: row.get(6)?,
                profile_arn: row.get(7)?,
                priority: row.get::<_, i64>(8)? as u32,
                disabled: row.get::<_, i64>(9)? != 0,
                failure_count: row.get::<_, i64>(10)? as u32,
                subscription_title: row.get(11)?,
                current_usage: row.get::<_, Option<f64>>(12)?.unwrap_or(0.0),
                usage_limit: row.get::<_, Option<f64>>(13)?.unwrap_or(0.0),
                next_reset_at: row.get(14)?,
                balance_updated_at: row.get(15)?,
                machine_id: row.get(16)?,
            })
        })?;

        let mut credentials = Vec::new();
        for row in rows {
            credentials.push(row?);
        }
        Ok(credentials)
    }

    /// 插入新凭据，返回分配的 ID
    pub fn insert_credential(&self, cred: &KiroCredentials) -> Result<u64> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            INSERT INTO credentials (refresh_token, access_token, expires_at, auth_method,
                                     client_id, client_secret, profile_arn, priority,
                                     disabled, failure_count,
                                     subscription_title, current_usage, usage_limit, next_reset_at, balance_updated_at,
                                     machine_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            "#,
            params![
                cred.refresh_token,
                cred.access_token,
                cred.expires_at,
                cred.auth_method,
                cred.client_id,
                cred.client_secret,
                cred.profile_arn,
                cred.priority as i64,
                cred.disabled as i64,
                cred.failure_count as i64,
                cred.subscription_title,
                cred.current_usage,
                cred.usage_limit,
                cred.next_reset_at,
                cred.balance_updated_at,
                cred.machine_id,
            ],
        )?;
        Ok(conn.last_insert_rowid() as u64)
    }

    /// 更新凭据
    pub fn update_credential(&self, cred: &KiroCredentials) -> Result<bool> {
        let id = cred.id.ok_or_else(|| anyhow::anyhow!("凭据缺少 ID"))?;
        let conn = self.conn.lock();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET refresh_token = ?1, access_token = ?2, expires_at = ?3, auth_method = ?4,
                client_id = ?5, client_secret = ?6, profile_arn = ?7, priority = ?8,
                disabled = ?9, failure_count = ?10,
                subscription_title = ?11, current_usage = ?12, usage_limit = ?13,
                next_reset_at = ?14, balance_updated_at = ?15, machine_id = ?16,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?17
            "#,
            params![
                cred.refresh_token,
                cred.access_token,
                cred.expires_at,
                cred.auth_method,
                cred.client_id,
                cred.client_secret,
                cred.profile_arn,
                cred.priority as i64,
                cred.disabled as i64,
                cred.failure_count as i64,
                cred.subscription_title,
                cred.current_usage,
                cred.usage_limit,
                cred.next_reset_at,
                cred.balance_updated_at,
                cred.machine_id,
                id as i64,
            ],
        )?;
        Ok(affected > 0)
    }

    /// 删除凭据
    pub fn delete_credential(&self, id: u64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute("DELETE FROM credentials WHERE id = ?1", params![id as i64])?;
        Ok(affected > 0)
    }

    /// 获取单个凭据
    pub fn get_credential(&self, id: u64) -> Result<Option<KiroCredentials>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, refresh_token, access_token, expires_at, auth_method,
                   client_id, client_secret, profile_arn, priority,
                   disabled, failure_count,
                   subscription_title, current_usage, usage_limit, next_reset_at, balance_updated_at,
                   machine_id
            FROM credentials
            WHERE id = ?1
            "#,
        )?;

        let result = stmt.query_row(params![id as i64], |row| {
            Ok(KiroCredentials {
                id: Some(row.get::<_, i64>(0)? as u64),
                refresh_token: row.get(1)?,
                access_token: row.get(2)?,
                expires_at: row.get(3)?,
                auth_method: row.get(4)?,
                client_id: row.get(5)?,
                client_secret: row.get(6)?,
                profile_arn: row.get(7)?,
                priority: row.get::<_, i64>(8)? as u32,
                disabled: row.get::<_, i64>(9)? != 0,
                failure_count: row.get::<_, i64>(10)? as u32,
                subscription_title: row.get(11)?,
                current_usage: row.get::<_, Option<f64>>(12)?.unwrap_or(0.0),
                usage_limit: row.get::<_, Option<f64>>(13)?.unwrap_or(0.0),
                next_reset_at: row.get(14)?,
                balance_updated_at: row.get(15)?,
                machine_id: row.get(16)?,
            })
        });

        match result {
            Ok(cred) => Ok(Some(cred)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 获取凭据数量
    pub fn count_credentials(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 =
            conn.query_row("SELECT COUNT(*) FROM credentials", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    /// 更新凭据余额信息
    pub fn update_balance(
        &self,
        id: u64,
        subscription_title: Option<&str>,
        current_usage: f64,
        usage_limit: f64,
        next_reset_at: Option<f64>,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET subscription_title = ?1, current_usage = ?2, usage_limit = ?3,
                next_reset_at = ?4, balance_updated_at = ?5, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?6
            "#,
            params![
                subscription_title,
                current_usage,
                usage_limit,
                next_reset_at,
                now,
                id as i64,
            ],
        )?;
        Ok(affected > 0)
    }

    /// 设置凭据禁用状态
    ///
    /// 禁用时记录 disabled_at 时间戳，启用时清除
    pub fn set_disabled(&self, id: u64, disabled: bool) -> Result<bool> {
        let conn = self.conn.lock();
        let disabled_at = if disabled {
            Some(chrono::Utc::now().to_rfc3339())
        } else {
            None
        };
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET disabled = ?1, disabled_at = ?2, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?3
            "#,
            params![disabled as i64, disabled_at, id as i64],
        )?;
        Ok(affected > 0)
    }

    /// 设置凭据优先级
    pub fn set_priority(&self, id: u64, priority: u32) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET priority = ?1, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            "#,
            params![priority as i64, id as i64],
        )?;
        Ok(affected > 0)
    }

    /// 增加失败计数
    pub fn increment_failure_count(&self, id: u64) -> Result<u32> {
        let conn = self.conn.lock();
        conn.execute(
            r#"
            UPDATE credentials
            SET failure_count = failure_count + 1, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
            params![id as i64],
        )?;
        let count: i64 = conn.query_row(
            "SELECT failure_count FROM credentials WHERE id = ?1",
            params![id as i64],
            |row| row.get(0),
        )?;
        Ok(count as u32)
    }

    /// 重置失败计数
    pub fn reset_failure_count(&self, id: u64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET failure_count = 0, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
            params![id as i64],
        )?;
        Ok(affected > 0)
    }

    /// 重置失败计数并启用凭据
    pub fn reset_and_enable(&self, id: u64) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET failure_count = 0, disabled = 0, disabled_at = NULL, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?1
            "#,
            params![id as i64],
        )?;
        Ok(affected > 0)
    }

    /// 尝试恢复冷却期已过的禁用凭据
    ///
    /// 返回恢复的凭据数量
    pub fn try_recover_disabled(&self, cooldown_seconds: i64) -> Result<usize> {
        let conn = self.conn.lock();
        let cutoff = chrono::Utc::now() - chrono::Duration::seconds(cooldown_seconds);
        let cutoff_str = cutoff.to_rfc3339();

        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET disabled = 0, disabled_at = NULL, failure_count = 0, updated_at = CURRENT_TIMESTAMP
            WHERE disabled = 1 AND disabled_at IS NOT NULL AND disabled_at < ?1
            "#,
            params![cutoff_str],
        )?;

        if affected > 0 {
            tracing::info!("已自动恢复 {} 个冷却期已过的凭据", affected);
        }

        Ok(affected)
    }

    /// 获取优先级最高的可用凭据
    pub fn get_highest_priority_available(&self) -> Result<Option<KiroCredentials>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, refresh_token, access_token, expires_at, auth_method,
                   client_id, client_secret, profile_arn, priority,
                   disabled, failure_count,
                   subscription_title, current_usage, usage_limit, next_reset_at, balance_updated_at,
                   machine_id
            FROM credentials
            WHERE disabled = 0
            ORDER BY priority ASC
            LIMIT 1
            "#,
        )?;

        let result = stmt.query_row([], |row| {
            Ok(KiroCredentials {
                id: Some(row.get::<_, i64>(0)? as u64),
                refresh_token: row.get(1)?,
                access_token: row.get(2)?,
                expires_at: row.get(3)?,
                auth_method: row.get(4)?,
                client_id: row.get(5)?,
                client_secret: row.get(6)?,
                profile_arn: row.get(7)?,
                priority: row.get::<_, i64>(8)? as u32,
                disabled: row.get::<_, i64>(9)? != 0,
                failure_count: row.get::<_, i64>(10)? as u32,
                subscription_title: row.get(11)?,
                current_usage: row.get::<_, Option<f64>>(12)?.unwrap_or(0.0),
                usage_limit: row.get::<_, Option<f64>>(13)?.unwrap_or(0.0),
                next_reset_at: row.get(14)?,
                balance_updated_at: row.get(15)?,
                machine_id: row.get(16)?,
            })
        });

        match result {
            Ok(cred) => Ok(Some(cred)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 获取下一个优先级最高的可用凭据（排除指定 ID）
    pub fn get_next_available(&self, exclude_id: u64) -> Result<Option<KiroCredentials>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, refresh_token, access_token, expires_at, auth_method,
                   client_id, client_secret, profile_arn, priority,
                   disabled, failure_count,
                   subscription_title, current_usage, usage_limit, next_reset_at, balance_updated_at,
                   machine_id
            FROM credentials
            WHERE disabled = 0 AND id != ?1
            ORDER BY priority ASC
            LIMIT 1
            "#,
        )?;

        let result = stmt.query_row(params![exclude_id as i64], |row| {
            Ok(KiroCredentials {
                id: Some(row.get::<_, i64>(0)? as u64),
                refresh_token: row.get(1)?,
                access_token: row.get(2)?,
                expires_at: row.get(3)?,
                auth_method: row.get(4)?,
                client_id: row.get(5)?,
                client_secret: row.get(6)?,
                profile_arn: row.get(7)?,
                priority: row.get::<_, i64>(8)? as u32,
                disabled: row.get::<_, i64>(9)? != 0,
                failure_count: row.get::<_, i64>(10)? as u32,
                subscription_title: row.get(11)?,
                current_usage: row.get::<_, Option<f64>>(12)?.unwrap_or(0.0),
                usage_limit: row.get::<_, Option<f64>>(13)?.unwrap_or(0.0),
                next_reset_at: row.get(14)?,
                balance_updated_at: row.get(15)?,
                machine_id: row.get(16)?,
            })
        });

        match result {
            Ok(cred) => Ok(Some(cred)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// 获取可用凭据数量
    pub fn count_available(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM credentials WHERE disabled = 0",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// 设置凭据的 machine_id
    #[allow(dead_code)]
    pub fn set_machine_id(&self, id: u64, machine_id: Option<&str>) -> Result<bool> {
        let conn = self.conn.lock();
        let affected = conn.execute(
            r#"
            UPDATE credentials
            SET machine_id = ?1, updated_at = CURRENT_TIMESTAMP
            WHERE id = ?2
            "#,
            params![machine_id, id as i64],
        )?;
        Ok(affected > 0)
    }

    /// 检查 client_id 是否已存在
    ///
    /// 用于添加凭据时去重，只检查非空的 client_id
    pub fn client_id_exists(&self, client_id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM credentials WHERE client_id = ?1",
            params![client_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_database_open_and_init() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();
        assert_eq!(db.count_credentials().unwrap(), 0);
    }

    #[test]
    fn test_insert_and_load() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let cred = KiroCredentials {
            id: None,
            refresh_token: Some("test_refresh".to_string()),
            access_token: Some("test_access".to_string()),
            expires_at: Some("2025-12-31T00:00:00Z".to_string()),
            auth_method: Some("social".to_string()),
            client_id: None,
            client_secret: None,
            profile_arn: None,
            machine_id: None,
            priority: 0,
            disabled: false,
            failure_count: 0,
            subscription_title: None,
            current_usage: 0.0,
            usage_limit: 0.0,
            next_reset_at: None,
            balance_updated_at: None,
        };

        let id = db.insert_credential(&cred).unwrap();
        assert!(id > 0);

        let loaded = db.load_credentials().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, Some(id));
        assert_eq!(loaded[0].refresh_token, Some("test_refresh".to_string()));
    }

    #[test]
    fn test_update_credential() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let mut cred = KiroCredentials {
            id: None,
            refresh_token: Some("original".to_string()),
            ..Default::default()
        };

        let id = db.insert_credential(&cred).unwrap();
        cred.id = Some(id);
        cred.refresh_token = Some("updated".to_string());

        assert!(db.update_credential(&cred).unwrap());

        let loaded = db.get_credential(id).unwrap().unwrap();
        assert_eq!(loaded.refresh_token, Some("updated".to_string()));
    }

    #[test]
    fn test_delete_credential() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        let cred = KiroCredentials {
            id: None,
            refresh_token: Some("to_delete".to_string()),
            ..Default::default()
        };

        let id = db.insert_credential(&cred).unwrap();
        assert_eq!(db.count_credentials().unwrap(), 1);

        assert!(db.delete_credential(id).unwrap());
        assert_eq!(db.count_credentials().unwrap(), 0);
    }

    #[test]
    fn test_priority_ordering() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        // 插入不同优先级的凭据
        for (token, priority) in [("high", 0), ("low", 2), ("medium", 1)] {
            let cred = KiroCredentials {
                id: None,
                refresh_token: Some(token.to_string()),
                priority,
                ..Default::default()
            };
            db.insert_credential(&cred).unwrap();
        }

        let loaded = db.load_credentials().unwrap();
        assert_eq!(loaded[0].refresh_token, Some("high".to_string()));
        assert_eq!(loaded[1].refresh_token, Some("medium".to_string()));
        assert_eq!(loaded[2].refresh_token, Some("low".to_string()));
    }
}
