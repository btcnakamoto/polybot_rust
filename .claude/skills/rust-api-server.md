# Skill: 后端 API 服务

## 概述
使用 Axum 构建 RESTful API + WebSocket 推送服务，为 React 仪表盘提供数据接口。

## 核心依赖
```toml
[dependencies]
axum = { version = "0.7", features = ["ws"] }
axum-extra = { version = "0.9", features = ["typed-header"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "compression-gzip"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
jsonwebtoken = "9"
```

## 项目结构
```
src/api/
├── mod.rs          # 路由聚合
├── router.rs       # 路由定义
├── handlers/
│   ├── whales.rs   # 巨鲸相关接口
│   ├── trades.rs   # 交易相关接口
│   ├── positions.rs # 持仓相关接口
│   ├── dashboard.rs # 仪表盘聚合数据
│   └── ws.rs       # WebSocket 推送
├── middleware/
│   ├── auth.rs     # JWT 认证中间件
│   └── logging.rs  # 请求日志
├── models/
│   ├── request.rs  # 请求体类型
│   └── response.rs # 响应体类型
└── errors.rs       # 统一错误处理
```

## 路由定义
```rust
use axum::{Router, routing::{get, post, delete}};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // 仪表盘
        .route("/api/dashboard/summary", get(handlers::dashboard::summary))
        .route("/api/dashboard/pnl-history", get(handlers::dashboard::pnl_history))
        // 巨鲸管理
        .route("/api/whales", get(handlers::whales::list))
        .route("/api/whales", post(handlers::whales::add))
        .route("/api/whales/{id}", get(handlers::whales::detail))
        .route("/api/whales/{id}", delete(handlers::whales::remove))
        .route("/api/whales/{id}/trades", get(handlers::whales::trades))
        // 跟单交易
        .route("/api/trades", get(handlers::trades::list))
        .route("/api/trades/active", get(handlers::trades::active))
        // 持仓
        .route("/api/positions", get(handlers::positions::list))
        .route("/api/positions/open", get(handlers::positions::open))
        // WebSocket 实时推送
        .route("/ws", get(handlers::ws::handler))
        // 中间件
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}
```

## AppState
```rust
use sqlx::PgPool;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub ws_tx: broadcast::Sender<WsMessage>,  // WebSocket 广播
    pub config: AppConfig,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum WsMessage {
    WhaleAlert(WhaleTradeEvent),
    OrderUpdate(OrderStatusUpdate),
    PositionUpdate(PositionSnapshot),
    PnlUpdate(PnlSnapshot),
}
```

## 统一响应格式
```rust
use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Json<Self> {
        Json(Self { success: true, data: Some(data), error: None })
    }
    pub fn err(msg: impl Into<String>) -> Json<ApiResponse<()>> {
        Json(ApiResponse { success: false, data: None, error: Some(msg.into()) })
    }
}
```

## WebSocket 推送
```rust
use axum::extract::ws::{WebSocket, WebSocketUpgrade};

pub async fn handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.ws_tx.subscribe();
    while let Ok(msg) = rx.recv().await {
        let json = serde_json::to_string(&msg).unwrap();
        if socket.send(axum::extract::ws::Message::Text(json)).await.is_err() {
            break;  // 客户端断开
        }
    }
}
```

## 错误处理
```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
    Unauthorized,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Internal(e) => {
                tracing::error!("Internal error: {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".into())
            }
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "Unauthorized".into()),
        };
        (status, Json(ApiResponse::<()> { success: false, data: None, error: Some(message) })).into_response()
    }
}
```

## 注意事项
- CORS 开发阶段设为 permissive，生产环境需限制 origin
- WebSocket 使用 broadcast channel，新连接自动接收实时数据
- 所有 Handler 的错误需映射为 `AppError`，禁止直接 unwrap
- API 分页使用 `?page=1&per_page=20` 参数
- 敏感操作 (添加/删除巨鲸、修改风控参数) 需要 JWT 认证
