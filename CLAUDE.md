# Polybot — Polymarket 巨鲸跟单机器人

## 项目概述
实时监控 Polymarket 巨鲸钱包交易行为，自动评分、筛选并执行跟单策略的全自动交易系统。

## 技术栈
- **后端**: Rust (tokio + axum + sqlx)
- **前端**: React + TypeScript + Vite
- **数据库**: PostgreSQL
- **缓存**: Redis (可选)
- **链**: Polygon (Polymarket 底层链)
- **合约标准**: ERC-1155 (CTF 代币), EIP-712 (签名)

## 项目结构
```
polybot_rust/
├── CLAUDE.md                    # 本文件 — 项目主指令
├── .claude/skills/              # AI 辅助开发技能文件
├── src/                         # Rust 后端源码
│   ├── main.rs
│   ├── config/                  # 配置管理
│   ├── api/                     # HTTP/WebSocket API 层
│   ├── services/                # 业务逻辑层
│   │   ├── whale_tracker.rs     # 巨鲸监控
│   │   ├── scorer.rs            # 钱包评分
│   │   ├── copy_engine.rs       # 跟单引擎
│   │   └── risk_manager.rs      # 风控模块
│   ├── models/                  # 数据模型
│   ├── db/                      # 数据库交互
│   └── utils/                   # 工具函数
├── dashboard/                   # React 前端
│   ├── src/
│   │   ├── components/          # UI 组件
│   │   ├── pages/               # 页面
│   │   ├── hooks/               # 自定义 hooks
│   │   ├── services/            # API 调用
│   │   └── types/               # TypeScript 类型
│   └── ...
├── migrations/                  # 数据库迁移
├── tests/                       # 集成测试
├── Cargo.toml
└── docker-compose.yml
```

## 编码规范

### Rust
- 使用 `thiserror` 定义错误类型，避免 `unwrap()` 在生产代码中出现
- 所有异步函数基于 `tokio` 运行时
- 日志使用 `tracing` crate，按 info/warn/error 分级
- 配置通过环境变量 + `.env` 文件管理 (`dotenvy`)
- 金额计算使用 `rust_decimal`，禁止浮点数

### React/TypeScript
- 函数组件 + Hooks，不使用 class 组件
- 状态管理: Zustand
- 样式: Tailwind CSS
- API 请求: TanStack Query (React Query)
- 图表: Lightweight Charts (TradingView) / Recharts

### Git
- 分支命名: `feat/xxx`, `fix/xxx`, `refactor/xxx`
- Commit message 使用英文，遵循 Conventional Commits
- 主分支: `main`

## Skills 索引
详见 `.claude/skills/` 目录：
1. `rust-async-networking.md` — 异步网络编程
2. `polymarket-api-integration.md` — Polymarket API 对接
3. `whale-tracking.md` — 巨鲸地址监控
4. `copy-trade-engine.md` — 跟单引擎
5. `rust-data-persistence.md` — 数据持久化
6. `rust-api-server.md` — 后端 API 服务
7. `react-dashboard.md` — React 仪表盘
8. `testing-strategy.md` — 测试策略
9. `deployment-ops.md` — 部署与运维

## 关键业务概念
- **CTF (Conditional Token Framework)**: Polymarket 使用的代币框架，每个市场 mint YES/NO 两种 ERC-1155 代币
- **CLOB**: Central Limit Order Book，Polymarket 的混合去中心化订单簿
- **巨鲸篮子 (Whale Basket)**: 按主题分组的高胜率钱包集合，当篮子内 >80% 钱包共识时触发跟单
- **Kelly Criterion**: 用于计算最优跟单仓位大小的数学公式
