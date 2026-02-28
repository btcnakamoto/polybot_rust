# Polybot 需求文档

> 基于 readme.md 提炼，版本 1.0

---

## 1. 项目定义

### 1.1 产品定位
Polymarket 巨鲸跟单机器人 — 实时监控链上巨鲸交易行为，通过量化评分筛选目标钱包，自动计算仓位并执行跟单交易。

### 1.2 核心前提
Polymarket 运行在 Polygon 链上，CLOB 链下撮合、链上结算 (EIP-712)，所有交易数据完全公开透明：
- 每笔买卖链上可查
- 每个钱包的完整历史可追溯
- CTF 合约为每个结果铸造 ERC-1155 的 YES/NO 代币

### 1.3 用户角色
| 角色 | 描述 |
|------|------|
| 运营者 (Operator) | 配置巨鲸列表、风控参数、跟单策略，通过仪表盘监控系统运行 |
| 系统 (Bot) | 自动执行巨鲸监控、评分、信号生成、下单的全流程 |

---

## 2. 功能需求

### FR-1: 巨鲸检测与分类

#### FR-1.1 交易检测
- **数据源**: Polymarket Data API + CLOB WebSocket + Polygon RPC
- **巨鲸阈值**: 单笔名义价值 (size × price) > **$10,000**
- **实时性**: WebSocket 实时监听，检测延迟 <1s

#### FR-1.2 钱包分类
系统必须将钱包自动归入以下三类：

| 类型 | 特征 | 处理方式 |
|------|------|----------|
| **Informed Trader** | 低频、高确信、$50k-$500k/笔、专注 1-2 类别、胜率 >60% (50+ 笔) | 纳入跟单候选 |
| **Market Maker** | 同一市场同时持有 YES 和 NO、靠价差盈利 | 过滤排除 |
| **Bot/Algo** | 日交易数百笔、交易间隔 <1s、利用 30-90s 延迟套利 | 过滤排除 |

#### FR-1.3 跟单准入门槛
钱包必须同时满足以下条件才可进入跟单列表：
- 历史交易 ≥ **100 笔**
- 活跃历史 ≥ **4 个月**
- 历史胜率 ≥ **60%**
- 非 Market Maker、非 Bot

---

### FR-2: 钱包评分系统

#### FR-2.1 Sharpe Ratio (风险调整收益)
```
Sharpe = (avg_return - risk_free_rate) / std_dev_of_returns
```
- risk_free_rate ≈ 0 (Polymarket 无无风险利率)
- **Sharpe > 1.5** = 优质钱包
- **Sharpe < 1.0** = 跳过

#### FR-2.2 Kelly Criterion (最优仓位比例)
```
f = (p × b - q) / b
```
- p = 历史胜率
- b = 平均赔率 (例: YES $0.40 → 赔率 2.5x → b = 1.5)
- q = 1 - p

#### FR-2.3 滚动胜率衰减检测
- 窗口: 最近 **30 笔** 交易
- 停止跟单条件 (任一触发):
  - 30 笔滚动胜率 < **55%**
  - 30 笔滚动胜率 < 全周期胜率 × **80%**

#### FR-2.4 期望值 (EV)
```
EV = (win_rate × avg_win) - (loss_rate × avg_loss)
EV_copy = EV - avg_slippage
```
- 仅跟单 EV > **$50/trade** 的钱包
- 预估滑点: **1-3%**

#### FR-2.5 评分更新
- 频率: 至少每 **24 小时** 全量更新一次
- 触发衰减时自动从跟单列表移除

---

### FR-3: 巨鲸篮子策略 (Whale Basket)

#### FR-3.1 篮子构建
- 按主题分类: 政治 (politics)、加密 (crypto)、体育 (sports)
- 每个篮子 **5-10 个** 评分合格的钱包
- 篮子内钱包准入条件:
  - 胜率 >60%
  - 历史 >4 个月
  - 过滤 bot: >100 trades/month = 自动化
  - 过滤 insider: 新账号 + <10 笔交易 + 巨额仓位 = 可疑

#### FR-3.2 共识信号触发
同时满足以下三个条件时生成跟单信号：
1. 篮子内 > **80%** 钱包买入同一方向 (YES 或 NO)
2. 买入行为发生在 **24-48 小时** 时间窗口内
3. 当前市场价格距离 resolution (0 或 1) 仍有 > **5¢** 空间

---

### FR-4: 反信号过滤 (Anti-Signals)

以下情况系统必须拒绝生成跟单信号：

| 反信号 | 原因 |
|--------|------|
| 排行榜前列账号 | 已被过度跟单，跟单链条过长，edge 消失 |
| BTC/ETH 市场的 bot 钱包 | 靠收窄价差盈利，跟单者买入时价差已被吃完 |
| 样本量 <100 笔或历史 <4 月 | 无法区分运气与实力 |
| 休眠账号突然大额交易 | 大概率是巨鲸马甲，历史数据不可靠 |

#### FR-4.1 马甲钱包识别
- 顶级交易者使用二级、三级钱包分散操作
- 需要跨账号行为模式匹配 (交易时间、市场偏好、仓位比例)
- 作为 V2 功能，MVP 阶段可手动标记关联钱包

---

### FR-5: 跟单执行引擎

#### FR-5.1 执行管道
```
Polygon RPC → WebSocket listener → Whale scorer → Kelly sizing → CLOB execution
```

#### FR-5.2 跟单策略 (可配置)
| 策略 | 描述 | 适用场景 |
|------|------|----------|
| **Proportional** | 按巨鲸仓位占其总资金的比例分配 | 资金量差距大 |
| **Fixed** | 固定金额 × 信号强度 | 初期使用，简单可控 |
| **Kelly** | 基于 Kelly Criterion 公式 + half-Kelly (×0.5) 降波动 | 有充足样本数据时 |

#### FR-5.3 订单类型
- **只使用 Limit Order**，禁止 Market Order (滑点不可控)
- 下单前获取当前 orderbook，以实际 best_ask/best_bid 作为 limit price

#### FR-5.4 延迟要求
- 从检测到巨鲸交易到提交订单: < **5 秒**
- 每晚 1 秒入场，价格恶化 **0.5-2%**
- 需持续监控: 实际成交价 vs 巨鲸入场价

#### FR-5.5 跟卖
- 巨鲸卖出时必须同步跟卖 (止盈/止损)

---

### FR-6: 风控模块

| 规则 | 默认值 | 说明 |
|------|--------|------|
| 单笔最大仓位占比 | **5%** | 占总资产比例 |
| 最大同时持仓数 | **2** | 来自 readme |
| 最小价差 (distance to resolution) | **5¢** | 价格太接近 0 或 1 时不入场 |
| 最大滑点容忍度 | **3%** | 超过则放弃执行 |
| 单钱包跟单权重上限 | **20%** | 避免过度依赖单个巨鲸 |
| 日最大亏损额 | 可配置 | 触发后当日停止交易 |

---

### FR-7: 仪表盘 (React Dashboard)

#### FR-7.1 主仪表盘页
- 资产总览: 总资产、总 PnL、今日 PnL
- 当前持仓列表 (实时更新)
- 最新巨鲸动态 (实时推送)
- PnL 历史曲线

#### FR-7.2 巨鲸管理页
- 巨鲸列表 (地址、分类、评分、胜率、状态)
- 单个巨鲸详情 (交易历史、评分指标、PnL 曲线)
- 手动添加/移除巨鲸

#### FR-7.3 交易记录页
- 所有跟单交易的历史记录
- 状态、策略、滑点、盈亏

#### FR-7.4 设置页
- 跟单策略选择与参数配置
- 风控参数配置

#### FR-7.5 实时性
- 关键数据通过 WebSocket 实时推送
- 非关键数据轮询 (30s 间隔)
- 显示 WebSocket 连接状态

---

## 3. 非功能需求

### NFR-1: 性能
- 巨鲸交易检测到下单完成: < 5 秒
- API 响应时间 (Dashboard): < 200ms (P95)
- WebSocket 推送延迟: < 100ms

### NFR-2: 可用性
- WebSocket 断线自动重连 (指数退避)
- 数据库连接池管理
- 所有外部 API 调用设超时 (10s)

### NFR-3: 安全
- 私钥、API Secret 仅通过环境变量管理，禁止硬编码
- Dashboard 访问需 JWT 认证
- 日志中禁止输出敏感信息
- 金额计算使用 Decimal，禁止浮点数

### NFR-4: 可观测性
- 结构化日志 (JSON 格式, tracing)
- 关键指标: 延迟、滑点、胜率、PnL
- 告警: WS 断线 >60s、执行延迟 >10s、日亏损达限额 80%

### NFR-5: 部署
- Docker 容器化，多阶段构建
- VPS 推荐荷兰 (Hetzner/OVH/DigitalOcean Amsterdam)
- Polygon 节点延迟 <1ms

---

## 4. 数据实体

| 实体 | 关键字段 |
|------|----------|
| **Whale** | address, category, classification, sharpe, win_rate, kelly_fraction, is_active |
| **WhaleBasket** | category, wallets[], consensus_threshold |
| **WhaleTrade** | whale_id, market_id, token_id, side, size, price, notional, tx_hash |
| **CopyOrder** | whale_trade_id, side, size, target_price, fill_price, slippage, status, strategy |
| **Position** | market_id, token_id, outcome, size, avg_entry_price, current_price, unrealized_pnl |
| **DailyPnL** | date, realized_pnl, unrealized_pnl, total_trades, win_trades, portfolio_value |

---

## 5. 外部依赖

| 依赖 | 用途 | 端点 |
|------|------|------|
| Polymarket CLOB API | 下单/撤单/查询 | `https://clob.polymarket.com` |
| Polymarket Data API | 历史交易数据 | `https://data-api.polymarket.com` |
| Polymarket WebSocket | 实时交易流 | `wss://ws-subscriptions-clob.polymarket.com/ws/market` |
| Gamma Markets API | 市场元数据 | `https://gamma-api.polymarket.com` |
| Polygon RPC | 链上数据/余额查询 | 可配置 (Alchemy/自建) |
| Polymarket CTF 合约 | ERC-1155 代币 | `0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E` |

---

## 6. MVP 范围

### P0 (MVP 必须)
- FR-1: 巨鲸检测 (WebSocket + Data API)
- FR-2: 钱包评分 (4 个指标)
- FR-5: 跟单执行 (Fixed 策略优先)
- FR-6: 基础风控 (仓位限制 + 持仓数限制)
- FR-7.1: 仪表盘主页 (资产概览 + 持仓)

### P1 (第二阶段)
- FR-3: 篮子策略 + 共识信号
- FR-5.2: Kelly 策略
- FR-7.2-7.4: 完整仪表盘
- FR-2.3: 自动衰减检测

### P2 (第三阶段)
- FR-4.1: 马甲钱包行为匹配
- 回测框架
- Prometheus 监控
- 多策略并行
