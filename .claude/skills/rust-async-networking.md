# Skill: Rust 异步网络编程

## 概述
本项目所有网络 I/O 均基于 Tokio 异步运行时，包括 HTTP 请求、WebSocket 连接和数据库操作。

## 核心依赖
```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "rustls-tls"] }
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-native-roots"] }
futures-util = "0.3"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

## 编码模式

### 1. HTTP 客户端 — 复用连接池
```rust
use reqwest::Client;
use std::time::Duration;

pub fn build_http_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to build HTTP client")
}
```
- 全局共享一个 `Client` 实例（通过 `Arc` 或 app state）
- 不要每次请求创建新 client

### 2. WebSocket — 自动重连模式
```rust
use tokio_tungstenite::connect_async;
use futures_util::{StreamExt, SinkExt};
use tokio::time::{sleep, Duration};

pub async fn ws_connect_with_retry(url: &str, max_retries: u32) {
    let mut retries = 0;
    loop {
        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                retries = 0;
                let (mut write, mut read) = ws_stream.split();
                while let Some(msg) = read.next().await {
                    match msg {
                        Ok(message) => { /* 处理消息 */ }
                        Err(e) => {
                            tracing::warn!("WebSocket error: {e}");
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                retries += 1;
                if retries > max_retries {
                    tracing::error!("Max retries reached, giving up");
                    return;
                }
                tracing::warn!("Connection failed ({retries}/{max_retries}): {e}");
            }
        }
        let delay = Duration::from_secs(2u64.pow(retries.min(5)));
        sleep(delay).await;
    }
}
```

### 3. 并发任务管理
```rust
use tokio::task::JoinSet;

let mut tasks = JoinSet::new();
for whale in whale_list {
    tasks.spawn(monitor_whale(whale.clone(), tx.clone()));
}

while let Some(result) = tasks.join_next().await {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::error!("Whale monitor error: {e}"),
        Err(e) => tracing::error!("Task panicked: {e}"),
    }
}
```

### 4. Channel 通信
```rust
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct WhaleTradeEvent {
    pub wallet: String,
    pub market_id: String,
    pub side: Side,
    pub size: Decimal,
    pub price: Decimal,
    pub timestamp: i64,
}

// 有界 channel，防止内存溢出
let (tx, mut rx) = mpsc::channel::<WhaleTradeEvent>(1000);
```

## 错误处理
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetworkError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
    #[error("Connection timeout after {0}s")]
    Timeout(u64),
    #[error("Rate limited, retry after {retry_after}s")]
    RateLimited { retry_after: u64 },
}
```

## 注意事项
- 所有网络调用必须设置超时，避免永久阻塞
- WebSocket 连接必须处理断线重连，使用指数退避
- 使用 `tracing` 记录所有网络错误，包含上下文信息
- Rate limit 响应 (429) 需解析 `Retry-After` 头并等待
- 生产环境使用 `rustls`，不依赖 OpenSSL
- 金额相关字段使用 `rust_decimal::Decimal`，禁止 f64
