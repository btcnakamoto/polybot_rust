# Skill: 跟单引擎

## 概述
跟单引擎是系统核心模块，负责接收巨鲸交易信号，计算跟单仓位，执行订单，并进行风控管理。

## 架构
来自 readme 的核心管道:
```
Polygon RPC → WebSocket listener → Whale scorer → Kelly sizing → CLOB execution
```

展开后的模块交互:
```
WhaleTradeEvent (from WebSocket listener)
    │
    ▼
┌─────────────┐    ┌──────────────┐    ┌───────────────┐
│ Whale Filter │───▶│ Kelly Sizer  │───▶│ CLOB Executor │
│ (scorer)     │    │ (position)   │    │ (order)       │
└─────────────┘    └──────────────┘    └───────────────┘
    │                                        │
    ▼                                        ▼
┌─────────────┐                      ┌───────────────┐
│ Risk Manager │◀─────────────────────│ Position Mgr  │
└─────────────┘                      └───────────────┘
```

## 跟单策略

### 策略 1: 比例跟单 (Proportional)
```rust
pub fn proportional_size(whale_size: Decimal, whale_bankroll: Decimal, my_bankroll: Decimal) -> Decimal {
    let whale_pct = whale_size / whale_bankroll;
    my_bankroll * whale_pct
}
```
- 按巨鲸仓位占其总资金比例来分配
- 适合资金量差距大的场景

### 策略 2: 固定额跟单 (Fixed)
```rust
pub fn fixed_size(base_amount: Decimal, signal_strength: Decimal) -> Decimal {
    base_amount * signal_strength  // signal_strength: 0.0 ~ 1.0
}
```
- 每次固定金额，按信号强度调整
- 简单可控，适合初期使用

### 策略 3: Kelly 最优仓位
```rust
pub fn kelly_size(bankroll: Decimal, win_rate: Decimal, avg_odds: Decimal, fraction: Decimal) -> Decimal {
    let kelly_f = (win_rate * avg_odds - (Decimal::ONE - win_rate)) / avg_odds;
    let kelly_f = kelly_f.max(Decimal::ZERO);  // 不做负仓位
    bankroll * kelly_f * fraction  // fraction 通常取 0.25-0.5 (quarter/half Kelly)
}
```
- 使用 half-Kelly (fraction=0.5) 降低波动
- 需要足够样本量 (>100 trades) 的胜率数据

## 风控模块

### 规则集
```rust
pub struct RiskLimits {
    pub max_position_pct: Decimal,         // 单笔最大仓位占比 (默认 5%)
    pub max_open_positions: usize,          // 最大同时持仓数 (默认 2)
    pub max_daily_loss: Decimal,            // 日最大亏损额
    pub max_single_market_exposure: Decimal, // 单市场最大敞口
    pub min_spread_to_resolution: Decimal,   // 最小价差 (距 0 或 1 的距离, 默认 0.05)
    pub max_slippage_pct: Decimal,           // 最大滑点容忍度 (默认 3%)
    pub cooldown_after_loss: Duration,       // 连续亏损后冷却期
}

pub fn check_risk(order: &PendingOrder, portfolio: &Portfolio, limits: &RiskLimits) -> Result<(), RiskViolation> {
    // 1. 单笔限额检查
    if order.size > portfolio.total_value * limits.max_position_pct {
        return Err(RiskViolation::PositionTooLarge);
    }
    // 2. 持仓数量检查
    if portfolio.open_positions >= limits.max_open_positions {
        return Err(RiskViolation::TooManyPositions);
    }
    // 3. 日亏损检查
    if portfolio.daily_pnl < -limits.max_daily_loss {
        return Err(RiskViolation::DailyLossExceeded);
    }
    // 4. 价差检查 — 价格太接近 0 或 1 没有空间
    let distance = order.price.min(Decimal::ONE - order.price);
    if distance < limits.min_spread_to_resolution {
        return Err(RiskViolation::SpreadTooNarrow);
    }
    Ok(())
}
```

## 订单执行
```rust
pub struct OrderExecutor {
    client: PolymarketClient,
    retry_config: RetryConfig,
}

impl OrderExecutor {
    pub async fn execute(&self, order: ValidatedOrder) -> Result<OrderResult, ExecutionError> {
        // 1. 预检查: 获取当前 orderbook 确认价格未偏离
        let book = self.client.get_orderbook(&order.token_id).await?;
        let current_price = match order.side {
            OrderSide::Buy => book.best_ask(),
            OrderSide::Sell => book.best_bid(),
        };

        // 2. 滑点检查
        let slippage = ((current_price - order.target_price) / order.target_price).abs();
        if slippage > order.max_slippage {
            return Err(ExecutionError::SlippageExceeded { expected: order.target_price, actual: current_price });
        }

        // 3. 下单 (使用 limit order 控制价格)
        let result = self.client.place_order(Order {
            token_id: order.token_id,
            side: order.side,
            price: current_price,
            size: order.size,
            order_type: OrderType::Limit,
        }).await?;

        // 4. 确认成交
        tracing::info!(order_id = %result.id, "Order placed successfully");
        Ok(result)
    }
}
```

## 跟单事件流
```
1. 收到 WhaleTradeEvent
2. 查询巨鲸评分 → 确认仍在跟单列表
3. 检查篮子共识 → 是否满足阈值
4. 计算仓位大小 (Kelly / Fixed / Proportional)
5. 风控检查 → 通过则继续
6. 获取当前市场价格 → 滑点检查
7. 执行 Limit Order
8. 记录交易到数据库
9. 更新持仓状态
10. 推送通知到 Dashboard (WebSocket)
```

## 注意事项
- 所有订单使用 Limit Order，不要用 Market Order (滑点不可控)
- 跟单延迟目标 <5s (从检测到巨鲸交易到下单)
- 使用 half-Kelly 而非 full-Kelly，降低破产风险
- 巨鲸卖出时也需要跟卖 (同步止盈/止损)
- 所有交易记录必须持久化，用于后续回测和评分更新
- 并发安全: 多个巨鲸同时触发时需要锁定风控检查
