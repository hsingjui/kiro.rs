# kiro-rs

一个用 Rust 编写的 Anthropic Claude API 兼容代理服务，将 Anthropic API 请求转换为 Kiro API 请求。

## 功能特性

- **Anthropic API 兼容**: 完整支持 Anthropic Claude API 格式
- **流式响应**: 支持 SSE (Server-Sent Events) 流式输出
- **Token 自动刷新**: 自动管理和刷新 OAuth Token
- **多凭据支持**: 支持配置多个凭据，按优先级自动故障转移
- **智能重试**: 单凭据最多重试 3 次，单请求最多重试 9 次
- **SQLite 存储**: 凭据存储在 SQLite 数据库中，支持原子操作和事务
- **Admin API**: 提供凭据管理 API，支持动态添加、删除、查询凭据
- **Thinking 模式**: 支持 Claude 的 extended thinking 功能
- **工具调用**: 完整支持 function calling / tool use
- **多模型支持**: 支持 Sonnet、Opus、Haiku 系列模型

## 支持的 API 端点

| 端点 | 方法 | 描述          |
|------|------|-------------|
| `/v1/models` | GET | 获取可用模型列表    |
| `/v1/messages` | POST | 创建消息（对话）    |
| `/v1/messages/count_tokens` | POST | 估算 Token 数量 |

### Admin API 端点

| 端点 | 方法 | 描述          |
|------|------|-------------|
| `/api/admin/credentials` | GET | 获取所有凭据状态 |
| `/api/admin/credentials` | POST | 添加新凭据 |
| `/api/admin/credentials/:id` | DELETE | 删除凭据 |
| `/api/admin/credentials/:id/disabled` | POST | 设置凭据禁用状态 |
| `/api/admin/credentials/:id/priority` | POST | 设置凭据优先级 |
| `/api/admin/credentials/:id/reset` | POST | 重置失败计数 |
| `/api/admin/credentials/:id/balance` | GET | 获取凭据余额 |

## 快速开始

### 1. 编译项目

```bash
cargo build --release
```

### 2. 配置文件

创建 `config.json` 配置文件：

```json
{
   "host": "127.0.0.1",   // 必配, 监听地址
   "port": 8990,  // 必配, 监听端口
   "apiKey": "sk-kiro-rs-qazWSXedcRFV123456",  // 必配, 请求的鉴权 token
   "region": "us-east-1",  // 必配, 区域, 一般保持默认即可
   "databasePath": "./kiro.db",  // 可选, SQLite 数据库路径, 默认 ./kiro.db
   "adminApiKey": "admin-secret-key",  // 可选, Admin API 密钥, 不配置则禁用 Admin API
   "kiroVersion": "0.8.0",  // 可选, 用于自定义请求特征, 不需要请删除: kiro ide 版本
   "systemVersion": "darwin#24.6.0",  // 可选, 用于自定义请求特征, 不需要请删除: 系统版本
   "nodeVersion": "22.21.1",  // 可选, 用于自定义请求特征, 不需要请删除: node 版本
   "countTokensApiUrl": "https://api.example.com/v1/messages/count_tokens", // 可选, 用于自定义token统计API, 不需要请删除
   "countTokensApiKey": "sk-your-count-tokens-api-key",  // 可选, 用于自定义token统计API, 不需要请删除
   "countTokensAuthType": "x-api-key",  // 可选, 用于自定义token统计API, 不需要请删除
   "proxyUrl": "http://127.0.0.1:7890", // 可选, HTTP/SOCK5代理, 不需要请删除
   "proxyUsername": "user",  // 可选, HTTP/SOCK5代理用户名, 不需要请删除
   "proxyPassword": "pass"  // 可选, HTTP/SOCK5代理密码, 不需要请删除
}
```
最小启动配置为:
```json
{
   "host": "127.0.0.1",
   "port": 8990,
   "apiKey": "sk-kiro-rs-qazWSXedcRFV123456",
   "region": "us-east-1"
}
```

### 3. 添加凭据

凭据存储在 SQLite 数据库中（默认路径 `./kiro.db`）。首次启动时数据库为空，需要通过 Admin API 添加凭据。

> **注意**: 需要在 `config.json` 中配置 `adminApiKey` 才能使用 Admin API。

#### 通过 Admin API 添加凭据

```bash
# 添加 Social 认证凭据
curl -X POST http://127.0.0.1:8990/api/admin/credentials \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-api-key" \
  -d '{
    "refreshToken": "你的刷新token",
    "authMethod": "social",
    "priority": 0
  }'

# 添加 IdC 认证凭据（带自定义机器码）
curl -X POST http://127.0.0.1:8990/api/admin/credentials \
  -H "Content-Type: application/json" \
  -H "x-api-key: your-admin-api-key" \
  -d '{
    "refreshToken": "你的刷新token",
    "authMethod": "idc",
    "clientId": "xxxxxxxxx",
    "clientSecret": "xxxxxxxxx",
    "machineId": "64位十六进制字符串（可选）",
    "priority": 1
  }'
```

> **多凭据特性说明**：
> - 按 `priority` 字段排序，数字越小优先级越高（默认为 0）
> - 每个凭据可以配置独立的 `machineId`（设备指纹），不配置则自动生成
> - 单凭据最多重试 3 次，单请求最多重试 9 次
> - 自动故障转移到下一个可用凭据
> - Token 刷新后自动持久化到数据库

### 4. 启动服务

```bash
./target/release/kiro-rs
```

或指定配置文件路径：

```bash
./target/release/kiro-rs -c /path/to/config.json
```

### 5. 使用 API

```bash
curl http://127.0.0.1:8990/v1/messages \
  -H "Content-Type: application/json" \
  -H "x-api-key: sk-your-custom-api-key" \
  -d '{
    "model": "claude-sonnet-4-20250514",
    "max_tokens": 1024,
    "messages": [
      {"role": "user", "content": "Hello, Claude!"}
    ]
  }'
```

## 配置说明

### config.json

| 字段 | 类型 | 默认值 | 描述                      |
|------|------|--------|-------------------------|
| `host` | string | `127.0.0.1` | 服务监听地址                  |
| `port` | number | `8080` | 服务监听端口                  |
| `apiKey` | string | - | 自定义 API Key（用于客户端认证）    |
| `region` | string | `us-east-1` | AWS 区域                  |
| `databasePath` | string | `./kiro.db` | SQLite 数据库路径（存储凭据） |
| `adminApiKey` | string | - | Admin API 密钥（不配置则禁用 Admin API） |
| `kiroVersion` | string | `0.8.0` | Kiro 版本号                |
| `systemVersion` | string | 随机 | 系统版本标识                  |
| `nodeVersion` | string | `22.21.1` | Node.js 版本标识            |
| `countTokensApiUrl` | string | - | 外部 count_tokens API 地址（可选） |
| `countTokensApiKey` | string | - | 外部 count_tokens API 密钥（可选） |
| `countTokensAuthType` | string | `x-api-key` | 外部 API 认证类型：`x-api-key` 或 `bearer` |
| `proxyUrl` | string | - | HTTP/SOCKS5 代理地址（可选） |
| `proxyUsername` | string | - | 代理用户名（可选） |
| `proxyPassword` | string | - | 代理密码（可选） |

### 凭据字段说明

凭据存储在 SQLite 数据库中，通过 Admin API 管理。

| 字段 | 类型 | 描述                      |
|------|------|-------------------------|
| `id` | number | 凭据唯一 ID（数据库自动分配）    |
| `refreshToken` | string | OAuth 刷新令牌（必填）              |
| `accessToken` | string | OAuth 访问令牌（可选，自动刷新）    |
| `profileArn` | string | AWS Profile ARN（可选，登录时返回） |
| `expiresAt` | string | Token 过期时间 (RFC3339)    |
| `authMethod` | string | 认证方式（social 或 idc，默认 social）      |
| `clientId` | string | IdC 登录的客户端 ID（IdC 认证必填）      |
| `clientSecret` | string | IdC 登录的客户端密钥（IdC 认证必填）      |
| `machineId` | string | 设备指纹（64位十六进制字符串，可选，不填则自动生成） |
| `priority` | number | 凭据优先级，数字越小越优先，默认为 0 |

## 模型映射

| Anthropic 模型 | Kiro 模型 |
|----------------|-----------|
| `*sonnet*` | `claude-sonnet-4.5` |
| `*opus*` | `claude-opus-4.5` |
| `*haiku*` | `claude-haiku-4.5` |

## 项目结构

```
kiro-rs/
├── src/
│   ├── main.rs                 # 程序入口
│   ├── model/                  # 配置和参数模型
│   │   ├── config.rs           # 应用配置
│   │   └── arg.rs              # 命令行参数
│   ├── anthropic/              # Anthropic API 兼容层
│   │   ├── router.rs           # 路由配置
│   │   ├── handlers.rs         # 请求处理器
│   │   ├── middleware.rs       # 认证中间件
│   │   ├── types.rs            # 类型定义
│   │   ├── converter.rs        # 协议转换器
│   │   ├── stream.rs           # 流式响应处理
│   │   └── token.rs            # Token 估算
│   ├── admin/                  # Admin API
│   │   ├── router.rs           # 路由配置
│   │   ├── handlers.rs         # 请求处理器
│   │   ├── middleware.rs       # 认证中间件
│   │   ├── service.rs          # 业务逻辑
│   │   ├── types.rs            # 类型定义
│   │   └── error.rs            # 错误处理
│   └── kiro/                   # Kiro API 客户端
│       ├── provider.rs         # API 提供者
│       ├── token_manager.rs    # Token 管理
│       ├── machine_id.rs       # 设备指纹生成
│       ├── db.rs               # SQLite 数据库
│       ├── model/              # 数据模型
│       │   ├── credentials.rs  # OAuth 凭证
│       │   ├── events/         # 响应事件类型
│       │   ├── requests/       # 请求类型
│       │   └── common/         # 共享类型
│       └── parser/             # AWS Event Stream 解析器
│           ├── decoder.rs      # 流式解码器
│           ├── frame.rs        # 帧解析
│           ├── header.rs       # 头部解析
│           └── crc.rs          # CRC 校验
├── Cargo.toml                  # 项目配置
└── config.example.json         # 配置示例
```

## 技术栈

- **Web 框架**: [Axum](https://github.com/tokio-rs/axum) 0.8
- **异步运行时**: [Tokio](https://tokio.rs/)
- **HTTP 客户端**: [Reqwest](https://github.com/seanmonstar/reqwest)
- **数据库**: [rusqlite](https://github.com/rusqlite/rusqlite) (SQLite)
- **序列化**: [Serde](https://serde.rs/)
- **日志**: [tracing](https://github.com/tokio-rs/tracing)
- **命令行**: [Clap](https://github.com/clap-rs/clap)

## 高级功能

### Thinking 模式

支持 Claude 的 extended thinking 功能：

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 16000,
  "thinking": {
    "type": "enabled",
    "budget_tokens": 10000
  },
  "messages": [...]
}
```

### 工具调用

完整支持 Anthropic 的 tool use 功能：

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "tools": [
    {
      "name": "get_weather",
      "description": "获取指定城市的天气",
      "input_schema": {
        "type": "object",
        "properties": {
          "city": {"type": "string"}
        },
        "required": ["city"]
      }
    }
  ],
  "messages": [...]
}
```

### 流式响应

设置 `stream: true` 启用 SSE 流式响应：

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 1024,
  "stream": true,
  "messages": [...]
}
```

## 认证方式

支持两种 API Key 认证方式：

1. **x-api-key Header**
   ```
   x-api-key: sk-your-api-key
   ```

2. **Authorization Bearer**
   ```
   Authorization: Bearer sk-your-api-key
   ```

## 环境变量

可通过环境变量配置日志级别：

```bash
RUST_LOG=debug ./target/release/kiro-rs
```

## 注意事项

1. **数据库安全**: 请妥善保管 SQLite 数据库文件（默认 `kiro.db`），其中包含敏感凭据
2. **Admin API 安全**: 建议为 `adminApiKey` 设置强密码，并限制 Admin API 的访问范围
3. **Token 刷新**: 服务会自动刷新过期的 Token，无需手动干预
4. **不支持的工具**: `web_search` 和 `websearch` 工具会被自动过滤

## License

MIT

## 致谢

本项目的实现离不开前辈的努力:  
 - [kiro2api](https://github.com/caidaoli/kiro2api)
 - [proxycast](https://github.com/aiclientproxy/proxycast)

本项目部分逻辑参考了以上的项目, 再次由衷的感谢!