# Polybot 架构设计

> 基于需求文档 v1.0

---

## 1. 系统全局架构

```
                         ┌─────────────────────────────────────────┐
                         │           External Services             │
                         │                                         │
                         │  ┌─────────┐ ┌──────────┐ ┌──────────┐ │
                         │  │Polymarket│ │Polymarket│ │ Polygon  │ │
                         │  │ CLOB API │ │ Data API │ │   RPC    │ │
                         │  └────┬─────┘ └────┬─────┘ └────┬─────┘ │
                         │       │             │            │       │
                         │  ┌────┴─────────────┴────────────┴────┐  │
                         │  │    Polymarket CLOB WebSocket       │  │
                         │  │  wss://ws-subscriptions-clob...    │  │
                         │  └────────────────┬───────────────────┘  │
                         └───────────────────┼─────────────────────┘
                                             │
                    ┌────────────────────────┼────────────────────────┐
                    │                        ▼                        │
                    │  ┌──────────────────────────────────────────┐   │
                    │  │          DATA INGESTION LAYER            │   │
                    │  │                                          │   │
                    │  │  WebSocket Listener ◄─── 实时交易流       │   │
                    │  │  Data API Poller    ◄─── 历史数据补全     │   │
                    │  │  RPC Client         ◄─── ERC-1155 余额   │   │
                    │  └──────────────┬───────────────────────────┘   │
                    │                 │ WhaleTradeEvent               │
                    │                 │ (mpsc channel)                │
                    │                 ▼                               │
                    │  ┌──────────────────────────────────────────┐   │
                    │  │          INTELLIGENCE LAYER              │   │
                    │  │                                          │   │
                    │  │  ┌─────────────┐    ┌────────────────┐   │   │
                    │  │  │ Whale Filter│───▶│ Wallet Scorer  │   │   │
                    │  │  │ (>$10k)     │    │ Sharpe/Kelly/  │   │   │
                    │  │  │ (whitelist) │    │ WR Decay/EV    │   │   │
                    │  │  └─────────────┘    └───────┬────────┘   │   │
                    │  │                             │             │   │
                    │  │                    ┌────────▼────────┐    │   │
                    │  │                    │ Basket Consensus│    │   │
                    │  │                    │ (>80% agree +   │    │   │
                    │  │                    │  24-48h window) │    │   │
                    │  │                    └────────┬────────┘    │   │
                    │  └────────────────────────────┼──────────────┘   │
                    │                               │ CopySignal      │
                    │                               ▼                 │
                    │  ┌──────────────────────────────────────────┐   │
                    │  │          EXECUTION LAYER                 │   │
                    │  │                                          │   │
                    │  │  ┌──────────┐  ┌──────────┐  ┌────────┐ │   │
                    │  │  │ Position │  │   Risk   │  │ Order  │ │   │
                    │  │  │  Sizer   │─▶│ Manager  │─▶│Executor│ │   │
                    │  │  │Kelly/Fix │  │ 5%/2pos/ │  │ CLOB   │ │   │
                    │  │  │/Propor.  │  │ 5¢/3%   │  │ Limit  │ │   │
                    │  │  └──────────┘  └──────────┘  └───┬────┘ │   │
                    │  └──────────────────────────────────┼──────┘   │
                    │                                     │          │
                    │                 ┌───────────────────┤          │
                    │                 │                   │          │
                    │                 ▼                   ▼          │
                    │  ┌──────────────────┐  ┌───────────────────┐  │
                    │  │   PostgreSQL     │  │  WebSocket Push   │  │
                    │  │  (持久化层)       │  │  (→ Dashboard)    │  │
                    │  └──────────────────┘  └───────────────────┘  │
                    │                                               │
                    │              POLYBOT RUST BACKEND              │
                    └───────────────────────────────────────────────┘
                                             │
                              WebSocket + REST API (axum)
                                             │
                    ┌────────────────────────┼────────────────────────┐
                    │                        ▼                        │
                    │  ┌──────────────────────────────────────────┐   │
                    │  │          REACT DASHBOARD                 │   │
                    │  │                                          │   │
                    │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ │   │
                    │  │  │ 资产概览  │ │ 巨鲸管理 │ │ 交易记录  │ │   │
                    │  │  │ PnL 曲线  │ │ 评分详情 │ │ 持仓面板  │ │   │
                    │  │  └──────────┘ └──────────┘ └──────────┘ │   │
                    │  └──────────────────────────────────────────┘   │
                    │              REACT + TYPESCRIPT + VITE          │
                    └────────────────────────────────────────────────┘
```

---

## 2. 核心数据流

### 2.1 主流程 — 从巨鲸交易到跟单执行

```
时间轴 ──────────────────────────────────────────────────────────────▶
                                                         目标 < 5s
 ┌──────┐   ┌───────┐   ┌───────┐   ┌──────┐   ┌──────┐   ┌──────┐
 │ WS   │──▶│ Parse │──▶│ Filter│──▶│ Score│──▶│ Size │──▶│ CLOB │
 │ Event│   │ Trade │   │ Whale │   │Check │   │+Risk │   │Order │
 └──────┘   └───────┘   └───────┘   └──────┘   └──────┘   └──┬───┘
                                                              │
               ┌──────────────────────────────────────────────┘
               ▼
 ┌──────┐   ┌───────┐   ┌──────────┐
 │  DB  │   │ WS    │   │ Position │
 │Record│   │ Push  │   │ Update   │
 └──────┘   └───────┘   └──────────┘
```

**各步骤详细说明:**

| 步骤 | 输入 | 处理 | 输出 | 耗时目标 |
|------|------|------|------|----------|
| 1. WS Event | WebSocket 原始消息 | 解析 JSON | WhaleTradeEvent | <10ms |
| 2. Filter | WhaleTradeEvent | 检查 notional >$10k + 地址在白名单 | FilteredTrade | <1ms |
| 3. Score Check | FilteredTrade | 查询钱包评分，确认未衰减 | ScoredTrade | <10ms |
| 4. Size + Risk | ScoredTrade | Kelly/Fixed 计算 + 风控 5 项检查 | ValidatedOrder | <5ms |
| 5. CLOB Order | ValidatedOrder | 获取 orderbook → 滑点检查 → Limit Order | OrderResult | <3s |
| 6. Persist | OrderResult | 写入 DB + 推送 Dashboard + 更新 Position | — | <100ms |

### 2.2 评分更新流程 (定时任务, 每 24h)

```
┌────────────┐    ┌────────────┐    ┌────────────┐    ┌────────────┐
│ Data API   │──▶ │ 拉取全部    │──▶ │ 计算 4 指标 │──▶ │ 更新 DB    │
│ 历史交易    │    │ 钱包交易    │    │ Sharpe/Kelly│    │ 衰减→移除  │
│            │    │            │    │ WR Decay/EV │    │ 新星→加入  │
└────────────┘    └────────────┘    └────────────┘    └────────────┘
```

### 2.3 篮子共识检测流程

```
                   ┌─────────────────┐
                   │ 收到巨鲸交易信号  │
                   └────────┬────────┘
                            ▼
                   ┌─────────────────┐
                   │ 该钱包属于哪个篮子?│
                   └────────┬────────┘
                            ▼
               ┌────────────────────────┐
               │ 查询该篮子在该市场的     │
               │ 同方向买入钱包数         │
               │ (24-48h 时间窗口内)     │
               └────────────┬───────────┘
                            ▼
                    ┌───────────────┐
                    │ 共识 > 80% ?  │
                    └──┬─────────┬──┘
                   YES │         │ NO
                       ▼         ▼
              ┌──────────┐  ┌──────────┐
              │ 检查价差   │  │ 不触发    │
              │ >5¢ ?     │  │ 等待更多  │
              └──┬─────┬──┘  │ 钱包加入  │
              YES│     │NO   └──────────┘
                 ▼     ▼
          ┌────────┐ ┌──────┐
          │生成跟单 │ │不入场 │
          │ 信号   │ │价差窄 │
          └────────┘ └──────┘
```

---

## 3. Rust 后端模块划分

```
src/
├── main.rs                          # 入口: 初始化各模块, 启动 tokio runtime
├── config/
│   └── mod.rs                       # 环境变量 + .env 配置
│
├── ingestion/                       # ── 数据摄入层 ──
│   ├── mod.rs
│   ├── ws_listener.rs               # WebSocket 连接 + 自动重连
│   ├── data_api.rs                  # Data API 客户端 (历史交易)
│   └── rpc_client.rs                # Polygon RPC (ERC-1155 余额)
│
├── intelligence/                    # ── 智能分析层 ──
│   ├── mod.rs
│   ├── classifier.rs                # 巨鲸分类 (Informed/MM/Bot)
│   ├── scorer.rs                    # 4 指标评分 (Sharpe/Kelly/WR/EV)
│   ├── decay_detector.rs            # 滚动胜率衰减检测
│   └── basket.rs                    # 篮子管理 + 共识信号生成
│
├── execution/                       # ── 执行层 ──
│   ├── mod.rs
│   ├── copy_engine.rs               # 跟单引擎主循环
│   ├── position_sizer.rs            # 仓位计算 (Kelly/Fixed/Proportional)
│   ├── risk_manager.rs              # 风控 5 项检查
│   ├── order_executor.rs            # CLOB API 下单
│   └── position_manager.rs          # 持仓状态管理
│
├── api/                             # ── Dashboard API 层 ──
│   ├── mod.rs
│   ├── router.rs                    # 路由聚合
│   ├── handlers/
│   │   ├── dashboard.rs             # 仪表盘聚合数据
│   │   ├── whales.rs                # 巨鲸 CRUD
│   │   ├── trades.rs                # 交易记录查询
│   │   ├── positions.rs             # 持仓查询
│   │   └── ws.rs                    # WebSocket 实时推送
│   ├── middleware/
│   │   └── auth.rs                  # JWT 认证
│   └── errors.rs                    # 统一错误响应
│
├── db/                              # ── 数据持久化 ──
│   ├── mod.rs
│   ├── whale_repo.rs
│   ├── trade_repo.rs
│   ├── position_repo.rs
│   └── pnl_repo.rs
│
├── models/                          # ── 数据模型 ──
│   ├── mod.rs
│   ├── whale.rs
│   ├── trade.rs
│   ├── order.rs
│   ├── position.rs
│   └── signal.rs
│
└── polymarket/                      # ── Polymarket SDK 封装 ──
    ├── mod.rs
    ├── auth.rs                      # HMAC + EIP-712 签名
    ├── clob_client.rs               # CLOB API 客户端
    ├── data_client.rs               # Data API 客户端
    └── types.rs                     # API 响应类型
```

---

## 4. 技术栈选型

### 4.1 后端

| 组件 | 选型 | 理由 |
|------|------|------|
| 语言 | Rust | 低延迟、内存安全、适合长时间运行的交易系统 |
| 异步运行时 | tokio | Rust 异步生态标准 |
| HTTP 框架 | axum | 类型安全、与 tokio 原生集成 |
| HTTP 客户端 | reqwest | 连接池、rustls |
| WebSocket | tokio-tungstenite | 异步 WebSocket |
| 数据库 | PostgreSQL + sqlx | 编译时 SQL 检查、异步 |
| 金额 | rust_decimal | 避免浮点精度问题 |
| 日志 | tracing | 结构化日志、span 追踪 |
| 签名 | ethers-rs / alloy | EIP-712 签名 |
| 错误 | thiserror + anyhow | 库用 thiserror，应用用 anyhow |
| 配置 | dotenvy | .env 文件管理 |

### 4.2 前端

| 组件 | 选型 | 理由 |
|------|------|------|
| 框架 | React 18 + TypeScript | 生态成熟 |
| 构建 | Vite | 快速 HMR |
| 状态管理 | Zustand | 轻量、简洁 |
| 数据请求 | TanStack Query | 缓存 + 轮询 + 乐观更新 |
| 图表 | Lightweight Charts + Recharts | TradingView K线 + 通用图表 |
| 样式 | Tailwind CSS | 暗色主题、快速开发 |

### 4.3 基础设施

| 组件 | 选型 |
|------|------|
| 容器 | Docker (多阶段构建) |
| 数据库 | PostgreSQL 16 |
| 缓存 | Redis 7 (可选) |
| 部署 | VPS 荷兰 (Hetzner/OVH) |

---

## 5. 数据库 ER 图

```
┌──────────────────┐       ┌──────────────────────┐
│     whales       │       │    whale_baskets      │
├──────────────────┤       ├──────────────────────┤
│ id           PK  │◄──┐   │ id               PK  │
│ address          │   │   │ category             │
│ label            │   │   │ consensus_threshold  │
│ category         │   │   └──────────┬───────────┘
│ classification   │   │              │
│ sharpe_ratio     │   │   ┌──────────┴───────────┐
│ win_rate         │   │   │  basket_wallets      │
│ total_trades     │   │   ├──────────────────────┤
│ total_pnl        │   ├──▶│ basket_id    FK      │
│ kelly_fraction   │   │   │ whale_id     FK      │
│ is_active        │   │   └─────────────────────-┘
│ last_trade_at    │   │
└────────┬─────────┘   │
         │             │
         │ 1:N         │
         ▼             │
┌──────────────────┐   │
│  whale_trades    │   │
├──────────────────┤   │
│ id           PK  │   │
│ whale_id     FK  │───┘
│ market_id        │
│ token_id         │
│ side             │
│ size             │
│ price            │
│ notional         │
│ tx_hash          │
│ traded_at        │
└────────┬─────────┘
         │
         │ 1:1
         ▼
┌──────────────────┐        ┌──────────────────┐
│  copy_orders     │        │   positions      │
├──────────────────┤        ├──────────────────┤
│ id           PK  │        │ id           PK  │
│ whale_trade_id FK│        │ market_id        │
│ market_id        │        │ token_id         │
│ token_id         │        │ outcome          │
│ side             │        │ size             │
│ size             │        │ avg_entry_price  │
│ target_price     │        │ current_price    │
│ fill_price       │        │ unrealized_pnl   │
│ slippage         │        │ realized_pnl     │
│ status           │        │ status           │
│ strategy         │        └──────────────────┘
└──────────────────┘
                            ┌──────────────────┐
                            │   daily_pnl      │
                            ├──────────────────┤
                            │ id           PK  │
                            │ date         UQ  │
                            │ realized_pnl     │
                            │ unrealized_pnl   │
                            │ total_trades     │
                            │ win_trades       │
                            │ portfolio_value  │
                            └──────────────────┘
```

---

## 6. 并发模型

```
tokio runtime
│
├── Task 1: WebSocket Listener (长生命周期)
│   └── 接收交易 → 解析 → 发送到 mpsc channel
│
├── Task 2: Copy Engine (长生命周期)
│   └── 从 channel 接收 → Filter → Score → Size → Risk → Execute
│
├── Task 3: Axum HTTP Server (长生命周期)
│   └── REST API + WebSocket 推送到 Dashboard
│
├── Task 4: Scorer Updater (定时, 每 24h)
│   └── 批量拉取历史数据 → 重算评分 → 更新 DB
│
├── Task 5: Position Updater (定时, 每 30s)
│   └── 获取当前市场价格 → 更新 unrealized PnL
│
└── JoinSet: Per-Market WS Subscriptions
    ├── Market A subscription
    ├── Market B subscription
    └── ...
```

**channel 通信:**
```
ws_listener ──mpsc──▶ copy_engine ──broadcast──▶ dashboard_ws
                           │
                           └──▶ db (sqlx)
```

---

## 7. 部署架构

```
┌─────────────────────────────────────────────┐
│              VPS (Netherlands)              │
│                                             │
│  ┌─────────┐  ┌──────────┐  ┌───────────┐  │
│  │ polybot │  │dashboard │  │  nginx    │  │
│  │ (Rust)  │  │ (React)  │  │ (reverse  │  │
│  │ :8080   │  │  :3000   │  │  proxy)   │  │
│  └────┬────┘  └────┬─────┘  └─────┬─────┘  │
│       │            │              │         │
│       └────────────┴──────────────┘         │
│                    │                        │
│  ┌─────────────────┴───────────────────┐    │
│  │         Docker Network              │    │
│  │  ┌──────────┐    ┌──────────┐       │    │
│  │  │PostgreSQL│    │  Redis   │       │    │
│  │  │  :5432   │    │  :6379   │       │    │
│  │  └──────────┘    └──────────┘       │    │
│  └─────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
         │
         │ HTTPS (443)
         ▼
    ┌──────────┐
    │  运营者   │
    │ Browser  │
    └──────────┘
```

---

## 8. MVP 实现顺序

```
Phase 0: 项目骨架
  └── Cargo.toml + axum hello world + DB 连接 + Docker

Phase 1: 数据通路 (P0)
  ├── polymarket/ SDK 封装 (auth + clob_client + data_client)
  ├── ingestion/ws_listener.rs (WebSocket 连接 + 解析)
  └── 验证: 能打印出实时交易日志

Phase 2: 智能层 (P0)
  ├── intelligence/scorer.rs (4 指标)
  ├── intelligence/classifier.rs (3 类分类)
  ├── db/ 持久化巨鲸 + 交易
  └── 验证: 能对已知钱包输出评分

Phase 3: 执行层 (P0)
  ├── execution/copy_engine.rs + position_sizer.rs + risk_manager.rs
  ├── execution/order_executor.rs (CLOB Limit Order)
  └── 验证: 能在测试网完成一笔跟单

Phase 4: Dashboard (P0)
  ├── api/ REST + WebSocket
  ├── dashboard/ React 主仪表盘
  └── 验证: 浏览器能看到实时持仓和 PnL

Phase 5: 篮子 + 完善 (P1)
  ├── intelligence/basket.rs
  ├── 完整仪表盘页面
  └── 自动衰减检测

Phase 6: 高级功能 (P2)
  ├── 马甲钱包识别
  ├── 回测框架
  └── Prometheus 监控
```
