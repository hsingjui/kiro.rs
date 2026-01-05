mod admin;
mod anthropic;
mod common;
mod http_client;
mod kiro;
mod model;
pub mod token;
mod web;

use std::sync::Arc;

use clap::Parser;
use kiro::db::Database;
use kiro::provider::KiroProvider;
use kiro::token_manager::MultiTokenManager;
use model::arg::Args;
use model::config::Config;

#[tokio::main]
async fn main() {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // 加载配置
    let config_path = args
        .config
        .unwrap_or_else(|| Config::default_config_path().to_string());
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        tracing::error!("加载配置失败: {}", e);
        std::process::exit(1);
    });

    // 打开 SQLite 数据库
    let db = Database::open(&config.database_path).unwrap_or_else(|e| {
        tracing::error!("打开数据库失败: {}", e);
        std::process::exit(1);
    });
    tracing::info!("数据库已打开: {}", config.database_path);

    // 获取 API Key
    let api_key = config.api_key.clone().unwrap_or_else(|| {
        tracing::error!("配置文件中未设置 apiKey");
        std::process::exit(1);
    });

    // 构建代理配置
    let proxy_config = config.proxy_url.as_ref().map(|url| {
        let mut proxy = http_client::ProxyConfig::new(url);
        if let (Some(username), Some(password)) = (&config.proxy_username, &config.proxy_password) {
            proxy = proxy.with_auth(username, password);
        }
        proxy
    });

    if proxy_config.is_some() {
        tracing::info!("已配置 HTTP 代理: {}", config.proxy_url.as_ref().unwrap());
    }

    // 创建 MultiTokenManager 和 KiroProvider
    let token_manager = MultiTokenManager::new(config.clone(), db.clone(), proxy_config.clone())
        .unwrap_or_else(|e| {
            tracing::error!("创建 Token 管理器失败: {}", e);
            std::process::exit(1);
        });

    let credentials_count = token_manager.total_count();
    if credentials_count == 0 {
        tracing::warn!("数据库中没有凭据，请通过 Admin API 添加凭据后使用");
    } else {
        tracing::info!("已加载 {} 个凭据", credentials_count);
    }

    // 获取第一个凭据用于日志显示
    let first_credentials = token_manager.credentials();

    let token_manager = Arc::new(token_manager);
    let kiro_provider = KiroProvider::with_proxy(token_manager.clone(), proxy_config.clone());

    // 初始化 count_tokens 配置
    token::init_config(token::CountTokensConfig {
        api_url: config.count_tokens_api_url.clone(),
        api_key: config.count_tokens_api_key.clone(),
        auth_type: config.count_tokens_auth_type.clone(),
        proxy: proxy_config,
    });

    // 构建 Anthropic API 路由（从第一个凭据获取 profile_arn）
    let anthropic_app = anthropic::create_router_with_provider(
        &api_key,
        Some(kiro_provider),
        first_credentials.profile_arn.clone(),
    );

    // 构建 Admin API 路由（如果配置了非空的 admin_api_key）
    // 安全检查：空字符串被视为未配置，防止空 key 绕过认证
    let admin_key_valid = config
        .admin_api_key
        .as_ref()
        .map(|k| !k.trim().is_empty())
        .unwrap_or(false);

    let app = if let Some(admin_key) = &config.admin_api_key {
        if admin_key.trim().is_empty() {
            tracing::warn!("admin_api_key 配置为空，Admin API 未启用");
            anthropic_app
        } else {
            let admin_service = admin::AdminService::new(token_manager.clone());
            let admin_state = admin::AdminState::new(admin_key, admin_service);
            let admin_app = admin::create_admin_router(admin_state);

            tracing::info!("Admin API 已启用");
            anthropic_app.nest("/api/admin", admin_app)
        }
    } else {
        anthropic_app
    };

    // 添加前端静态文件服务（作为 fallback，避免覆盖 API 路由）
    let web_router = web::create_web_router();
    let app = app.fallback_service(web_router);

    // 启动服务器
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("启动 Anthropic API 端点: {}", addr);
    tracing::info!("API Key: {}***", &api_key[..(api_key.len() / 2)]);
    tracing::info!("可用 API:");
    tracing::info!("  GET  /v1/models");
    tracing::info!("  POST /v1/messages");
    tracing::info!("  POST /v1/messages/count_tokens");
    if admin_key_valid {
        tracing::info!("Admin API:");
        tracing::info!("  GET  /api/admin/credentials");
        tracing::info!("  POST /api/admin/credentials/:id/disabled");
        tracing::info!("  POST /api/admin/credentials/:id/priority");
        tracing::info!("  POST /api/admin/credentials/:id/reset");
        tracing::info!("  GET  /api/admin/credentials/:id/balance");
        tracing::info!("  POST /api/admin/credentials");
        tracing::info!("  DELETE /api/admin/credentials/:id");
    }
    tracing::info!("Web UI: http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
