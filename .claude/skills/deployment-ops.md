# Skill: 部署与运维

## 概述
使用 Docker 容器化部署，支持本地开发和生产环境，包含监控告警和日志管理。

## Docker 配置

### Rust 后端 — 多阶段构建
```dockerfile
# Dockerfile
FROM rust:1.82 AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/polybot /usr/local/bin/
EXPOSE 8080
CMD ["polybot"]
```

### React 前端
```dockerfile
# dashboard/Dockerfile
FROM node:20-alpine AS builder
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
```

### docker-compose.yml
```yaml
version: '3.8'
services:
  polybot:
    build: .
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=postgres://polybot:password@db:5432/polybot
      - RUST_LOG=info
      - POLYMARKET_API_KEY=${POLYMARKET_API_KEY}
      - POLYMARKET_API_SECRET=${POLYMARKET_API_SECRET}
      - POLYMARKET_PASSPHRASE=${POLYMARKET_PASSPHRASE}
      - WALLET_PRIVATE_KEY=${WALLET_PRIVATE_KEY}
    depends_on:
      db:
        condition: service_healthy

  dashboard:
    build: ./dashboard
    ports:
      - "3000:80"
    depends_on:
      - polybot

  db:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: polybot
      POSTGRES_USER: polybot
      POSTGRES_PASSWORD: password
    volumes:
      - pgdata:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U polybot"]
      interval: 5s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

volumes:
  pgdata:
```

## 环境变量

### .env.example
```env
# Database
DATABASE_URL=postgres://polybot:password@localhost:5432/polybot

# Logging
RUST_LOG=info

# Polymarket API
POLYMARKET_API_KEY=
POLYMARKET_API_SECRET=
POLYMARKET_PASSPHRASE=

# Wallet (Polygon)
WALLET_PRIVATE_KEY=
RPC_URL=https://polygon-rpc.com

# Server
HOST=0.0.0.0
PORT=8080

# JWT
JWT_SECRET=

# Redis (optional)
REDIS_URL=redis://localhost:6379
```

## 日志配置
```rust
use tracing_subscriber::{fmt, EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub fn init_tracing() {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())  // RUST_LOG 控制
        .with(fmt::layer().json())             // 生产环境 JSON 格式
        .init();
}
```
- 开发环境: `RUST_LOG=polybot=debug`
- 生产环境: `RUST_LOG=polybot=info,tower_http=warn`

## 健康检查
```rust
// GET /health
pub async fn health_check(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1").execute(&state.db).await.is_ok();
    if db_ok {
        (StatusCode::OK, Json(json!({ "status": "healthy" })))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "status": "unhealthy", "db": "disconnected" })))
    }
}
```

## 监控告警
- **系统指标**: 使用 `metrics` crate 暴露 Prometheus 端点
- **关键告警**:
  - WebSocket 连接断开超过 60s
  - 跟单执行延迟 >10s
  - 日亏损达到限额的 80%
  - 数据库连接池耗尽
  - API 连续 5 次请求失败

## 生产部署检查清单
- [ ] 私钥和 API Secret 仅通过环境变量注入，不存在于代码或镜像中
- [ ] 数据库密码不使用默认值
- [ ] CORS 配置限定前端域名
- [ ] HTTPS 已配置 (通过 Nginx 或云负载均衡)
- [ ] 数据库自动备份已启用
- [ ] 日志不包含敏感信息 (私钥、API Secret)
- [ ] 风控参数已设置合理上限

## 延迟与 VPS 建议
- **延迟成本**: 每晚 1 秒入场，价格恶化 0.5-2% (来自 readme)
- **跟单延迟目标**: 检测到巨鲸交易后 <5s 完成下单
- **监控**: 持续追踪实际成交价 vs 巨鲸入场价，量化滑点
- 地理位置: 荷兰 (距离 Polygon 节点最近，延迟 <1ms)
- 配置: 2 vCPU / 4GB RAM / 40GB SSD 起步
- 提供商: Hetzner, OVH, DigitalOcean (Amsterdam)

## 注意事项
- 永远不要把 `.env` 文件提交到 Git
- Docker 镜像中不包含 `.env` 文件
- 生产环境使用 `cargo build --release`，debug 模式性能差 10-50x
- 数据库迁移在部署前手动执行，不在启动时自动运行
- 保持容器无状态，所有状态存储在 PostgreSQL 和 Redis
