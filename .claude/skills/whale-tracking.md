# Skill: 巨鲸地址监控

## 概述
通过链上数据和 Polymarket Data API 实时追踪高胜率巨鲸钱包的交易行为，构建钱包评分体系，为跟单引擎提供信号。

## 巨鲸分类

### 值得跟单的 — Informed Traders
- 交易频率低，高确信度，单笔金额大 ($50k-$500k)
- 专注 1-2 个类别（政治、加密、地缘政治）
- 识别条件: 胜率 >60%，样本量 50+ 笔
- 跟单准入: 至少 100 笔交易 + 4 个月以上历史（区分运气与实力）
- 持仓周期较长，不做日内交易

### 需要过滤的 — Market Makers
- 同时持有 YES 和 NO 头寸
- 靠买卖价差盈利，无方向性
- 识别方式: 同一市场双边持仓 + 高频小额交易

### 需要过滤的 — Bot/Algo Traders
- 日交易量数百笔，高频操作
- 利用 30-90 秒延迟套利（如 15 分钟加密市场），人类无法复制其速度优势
- 其 99.5% 胜率在人类延迟下会消失
- 识别方式: 交易间隔极短 (<1s) + 固定模式
- 篮子过滤阈值: >100 trades/month = 大概率是自动化

## 钱包评分模型

### Metric 1: Sharpe Ratio (风险调整收益)
```rust
pub fn sharpe_ratio(returns: &[Decimal]) -> Decimal {
    let n = Decimal::from(returns.len());
    let mean = returns.iter().sum::<Decimal>() / n;
    let variance = returns.iter()
        .map(|r| (r - mean).powi(2))
        .sum::<Decimal>() / n;
    let std_dev = variance.sqrt().unwrap_or(Decimal::ONE);
    mean / std_dev  // risk_free_rate ≈ 0
}
// Sharpe > 1.5 = 优质钱包, < 1.0 = 跳过
```

### Metric 2: Kelly Criterion (最优仓位)
```rust
pub fn kelly_fraction(win_rate: Decimal, avg_odds: Decimal) -> Decimal {
    // f = (p * b - q) / b
    let q = Decimal::ONE - win_rate;
    (win_rate * avg_odds - q) / avg_odds
}
```

### Metric 3: 滚动胜率衰减检测
```rust
pub fn rolling_win_rate(trades: &[TradeResult], window: usize) -> Decimal {
    let recent = &trades[trades.len().saturating_sub(window)..];
    let wins = recent.iter().filter(|t| t.profit > Decimal::ZERO).count();
    Decimal::from(wins) / Decimal::from(recent.len())
}

pub fn is_decaying(trades: &[TradeResult]) -> bool {
    let alltime_wr = rolling_win_rate(trades, trades.len());
    let recent_wr = rolling_win_rate(trades, 30);
    recent_wr < Decimal::new(55, 2) || recent_wr < alltime_wr * Decimal::new(80, 2)
}
// 30 笔滚动胜率 < 55% 或 < 全周期胜率的 80% → 停止跟单
```

### Metric 4: 期望值 (EV)
```rust
pub fn expected_value(trades: &[TradeResult]) -> Decimal {
    let wins: Vec<_> = trades.iter().filter(|t| t.profit > Decimal::ZERO).collect();
    let losses: Vec<_> = trades.iter().filter(|t| t.profit <= Decimal::ZERO).collect();

    let win_rate = Decimal::from(wins.len()) / Decimal::from(trades.len());
    let avg_win = wins.iter().map(|t| t.profit).sum::<Decimal>() / Decimal::from(wins.len());
    let avg_loss = losses.iter().map(|t| t.profit.abs()).sum::<Decimal>() / Decimal::from(losses.len());

    win_rate * avg_win - (Decimal::ONE - win_rate) * avg_loss
}
// EV > $50/trade 才值得跟单，需扣除 1-3% 预估滑点
```

## 巨鲸篮子策略 (Whale Basket)
```rust
pub struct WhaleBasket {
    pub category: String,           // "politics", "crypto", "sports"
    pub wallets: Vec<ScoredWallet>,  // 5-10 个高评分钱包
    pub consensus_threshold: Decimal, // 0.8 = 80% 共识
}

pub struct BasketSignal {
    pub market_id: String,
    pub outcome: String,            // "Yes" or "No"
    pub agreeing_wallets: usize,
    pub total_wallets: usize,
    pub avg_entry_price: Decimal,
    pub signal_strength: Decimal,   // agreeing / total
}

// 篮子钱包准入条件 (来自 readme Step 3):
// - 胜率 >60%
// - 历史 >4 个月
// - 过滤 bot: >100 trades/month = 大概率自动化
// - 过滤 insider: 新账号 + <10 笔交易 + 巨额仓位 = 可疑

// 触发条件:
// 1. 篮子内 >80% 钱包买入同一方向
// 2. 买入行为在 24-48 小时窗口内
// 3. 当前市场价格距离 resolution 仍有 >5¢ 空间
```

## 数据获取流程
1. **历史数据**: Data API → `GET /trades?maker_address={wallet}` 拉取历史交易
2. **实时监控**: WebSocket 订阅目标市场 → 过滤目标钱包地址
3. **链上补充**: Polygon RPC → 查询 ERC-1155 余额确认实际持仓

## 反信号 (Anti-Signals)
- 排行榜前列账号 — 所有人都在跟，你在跟一个跟单者的跟单者，edge 已消失
- 加密市场 (BTC/ETH) 的 bot 钱包 — 靠收窄价差盈利，你买入时 bot 已吃完价差
- 样本量 <100 笔、历史 <4 个月的钱包 — 无法区分运气与实力
- 马甲钱包识别 (cat-and-mouse game):
  - 顶级交易者现在使用二级、三级钱包分散操作
  - 休眠账号突然投入六位数 = 大概率是巨鲸的小号
  - 需要跨账号行为模式匹配 (交易时间、市场偏好、仓位比例)

## 巨鲸交易检测阈值
- 单笔名义价值 (notional = size × price) > $10,000 视为巨鲸级交易
- 通过 Data API 或 WebSocket 过滤时使用此阈值

## 注意事项
- 巨鲸评分至少每 24 小时更新一次
- 检测到胜率衰减时自动从跟单列表移除
- 单个钱包的跟单权重不超过总仓位的 20%
- 需要处理巨鲸使用多钱包分散交易的情况（行为模式匹配）
