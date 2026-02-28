# Skill: 数据持久化

## 概述
使用 PostgreSQL 存储所有业务数据，通过 `sqlx` 进行类型安全的异步数据库操作，Redis 作为可选缓存层。

## 核心依赖
```toml
[dependencies]
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "postgres", "chrono", "decimal", "uuid"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
rust_decimal = { version = "1", features = ["db-postgres", "serde"] }
```

## 数据库 Schema

### 巨鲸钱包表
```sql
CREATE TABLE whales (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    address VARCHAR(42) NOT NULL UNIQUE,
    label VARCHAR(100),
    category VARCHAR(50),         -- politics, crypto, sports
    classification VARCHAR(20),   -- informed, market_maker, bot
    sharpe_ratio DECIMAL(10,4),
    win_rate DECIMAL(5,4),
    total_trades INT DEFAULT 0,
    total_pnl DECIMAL(18,6),
    kelly_fraction DECIMAL(5,4),
    is_active BOOLEAN DEFAULT true,
    last_trade_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_whales_active ON whales(is_active) WHERE is_active = true;
CREATE INDEX idx_whales_category ON whales(category);
```

### 巨鲸交易记录表
```sql
CREATE TABLE whale_trades (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    whale_id UUID REFERENCES whales(id),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    side VARCHAR(4) NOT NULL,        -- BUY / SELL
    size DECIMAL(18,6) NOT NULL,
    price DECIMAL(10,6) NOT NULL,
    notional DECIMAL(18,6) NOT NULL, -- size * price
    tx_hash VARCHAR(66),
    block_number BIGINT,
    traded_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX idx_whale_trades_whale ON whale_trades(whale_id, traded_at DESC);
CREATE INDEX idx_whale_trades_market ON whale_trades(market_id);
```

### 跟单订单表
```sql
CREATE TABLE copy_orders (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    whale_trade_id UUID REFERENCES whale_trades(id),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    side VARCHAR(4) NOT NULL,
    size DECIMAL(18,6) NOT NULL,
    target_price DECIMAL(10,6) NOT NULL,
    fill_price DECIMAL(10,6),
    slippage DECIMAL(10,6),
    status VARCHAR(20) NOT NULL DEFAULT 'pending',  -- pending, filled, partial, cancelled, failed
    strategy VARCHAR(20) NOT NULL,                   -- proportional, fixed, kelly
    error_message TEXT,
    placed_at TIMESTAMPTZ DEFAULT NOW(),
    filled_at TIMESTAMPTZ
);

CREATE INDEX idx_copy_orders_status ON copy_orders(status) WHERE status IN ('pending', 'partial');
```

### 持仓表
```sql
CREATE TABLE positions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    market_id VARCHAR(100) NOT NULL,
    token_id VARCHAR(100) NOT NULL,
    outcome VARCHAR(10) NOT NULL,      -- Yes / No
    size DECIMAL(18,6) NOT NULL,
    avg_entry_price DECIMAL(10,6) NOT NULL,
    current_price DECIMAL(10,6),
    unrealized_pnl DECIMAL(18,6),
    status VARCHAR(10) DEFAULT 'open', -- open, closed
    opened_at TIMESTAMPTZ DEFAULT NOW(),
    closed_at TIMESTAMPTZ,
    realized_pnl DECIMAL(18,6)
);

CREATE INDEX idx_positions_open ON positions(status) WHERE status = 'open';
```

### 每日损益表
```sql
CREATE TABLE daily_pnl (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    date DATE NOT NULL UNIQUE,
    realized_pnl DECIMAL(18,6) DEFAULT 0,
    unrealized_pnl DECIMAL(18,6) DEFAULT 0,
    total_trades INT DEFAULT 0,
    win_trades INT DEFAULT 0,
    portfolio_value DECIMAL(18,6),
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

## sqlx 查询模式
```rust
// 编译时类型检查的查询
let whale = sqlx::query_as!(
    Whale,
    "SELECT * FROM whales WHERE address = $1 AND is_active = true",
    address
)
.fetch_optional(&pool)
.await?;

// 插入交易记录
sqlx::query!(
    r#"INSERT INTO whale_trades (whale_id, market_id, token_id, side, size, price, notional, traded_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    whale_id, market_id, token_id, side, size, price, notional, traded_at
)
.execute(&pool)
.await?;
```

## 数据库迁移
```bash
# 使用 sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres
sqlx database create
sqlx migrate add init_schema
sqlx migrate run
```
- 迁移文件放在 `migrations/` 目录
- 每次 schema 变更创建新的迁移文件，不修改已有迁移

## 注意事项
- 金额字段统一使用 `DECIMAL(18,6)` 对应 USDC 6 位精度
- 开启编译时查询检查: 设置 `DATABASE_URL` 环境变量
- 连接池配置: `max_connections=10`，根据负载调整
- 索引策略: 对高频查询的 WHERE 条件建索引
- 敏感数据 (API Key 等) 不存数据库，使用环境变量
