//! 前端静态文件服务模块
//!
//! 使用 rust-embed 在编译时将前端资源嵌入二进制文件

use axum::{
    Router,
    body::Body,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use rust_embed::Embed;

/// 嵌入 web/dist 目录下的所有前端静态文件
#[derive(Embed)]
#[folder = "web/dist"]
struct Assets;

/// 处理静态文件请求
async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    serve_file(&path)
}

/// 处理根路径请求，返回 index.html
async fn serve_index() -> impl IntoResponse {
    serve_file("index.html")
}

/// 从嵌入的资源中获取文件
fn serve_file(path: &str) -> Response {
    match Assets::get(path) {
        Some(content) => {
            // 根据文件扩展名猜测 MIME 类型
            let mime = mime_guess::from_path(path)
                .first_or_octet_stream()
                .to_string();

            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=31536000") // 1 年缓存
                .body(Body::from(content.data.into_owned()))
                .unwrap()
        }
        None => {
            // 对于 SPA，非静态资源路径返回 index.html
            if !path.contains('.') && let Some(index) = Assets::get("index.html") {
                let mime = "text/html; charset=utf-8";
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, mime)
                    .header(header::CACHE_CONTROL, "no-cache") // index.html 不缓存
                    .body(Body::from(index.data.into_owned()))
                    .unwrap();
            }

            Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("404 Not Found"))
                .unwrap()
        }
    }
}

/// 创建前端静态文件路由
pub fn create_web_router() -> Router {
    Router::new()
        .route("/", get(serve_index))
        .route("/{*path}", get(serve_static))
}
