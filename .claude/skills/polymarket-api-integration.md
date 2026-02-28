# Skill: Polymarket API 对接

## 概述
Polymarket 提供多层 API 接口：CLOB API (下单交易)、Data API (历史数据)、WebSocket (实时流) 以及链上合约交互 (Polygon)。

## API 端点

### CLOB API (交易核心)
- **Base URL**: `https://clob.polymarket.com`
- **认证方式**: API Key + API Secret + Passphrase，通过 HMAC 签名
- **主要端点**:
  - `GET /markets` — 获取市场列表
  - `GET /book` — 获取订单簿
  - `POST /order` — 下单
  - `DELETE /order/{id}` — 撤单
  - `GET /orders` — 查询订单
  - `GET /positions` — 查询持仓

### Data API (数据查询)
- **Base URL**: `https://data-api.polymarket.com`
- **无需认证**
- **主要端点**:
  - `GET /trades` — 历史交易记录
  - `GET /markets` — 市场信息
  - `GET /prices-history` — 价格历史

### Gamma Markets API
- **Base URL**: `https://gamma-api.polymarket.com`
- **主要端点**:
  - `GET /markets` — 市场元数据（标题、描述、分类）
  - `GET /events` — 事件信息

### WebSocket 实时流
- **URL**: `wss://ws-subscriptions-clob.polymarket.com/ws/market`
- **订阅消息格式**:
```json
{
    "type": "subscribe",
    "channel": "market",
    "assets_id": "<token-id>"
}
```

## 认证与签名

### API Key 认证
```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine, engine::general_purpose::STANDARD};

pub struct PolymarketAuth {
    pub api_key: String,
    pub api_secret: String,
    pub passphrase: String,
}

impl PolymarketAuth {
    pub fn sign(&self, timestamp: &str, method: &str, path: &str, body: &str) -> String {
        let message = format!("{timestamp}{method}{path}{body}");
        let secret_bytes = STANDARD.decode(&self.api_secret).unwrap();
        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes).unwrap();
        mac.update(message.as_bytes());
        STANDARD.encode(mac.finalize().into_bytes())
    }
}
```

### EIP-712 签名 (链上交易)
```rust
// Polymarket 使用 EIP-712 结构化签名进行链上订单
// 需要 ethers-rs 或 alloy 来构建签名
use ethers::signers::{LocalWallet, Signer};
use ethers::types::transaction::eip712::Eip712;
```

## 核心数据结构
```rust
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Market {
    pub condition_id: String,
    pub question: String,
    pub tokens: Vec<Token>,
    pub active: bool,
    pub closed: bool,
    pub volume: Decimal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Token {
    pub token_id: String,
    pub outcome: String,   // "Yes" or "No"
    pub price: Decimal,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub market: String,
    pub asset_id: String,
    pub side: String,       // "BUY" or "SELL"
    pub size: Decimal,
    pub price: Decimal,
    pub timestamp: i64,
    pub maker_address: String,
    pub taker_address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Order {
    pub token_id: String,
    pub side: OrderSide,
    pub price: Decimal,
    pub size: Decimal,
    pub order_type: OrderType,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OrderSide { Buy, Sell }

#[derive(Debug, Serialize, Deserialize)]
pub enum OrderType { Limit, Market }
```

## 链上合约
```
Polymarket CTF 合约 (Polygon):
  主合约: 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E
  Exchange: 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E
  USDC (Polygon): 0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174
```

## Rate Limit
- CLOB API: 约 10 req/s
- Data API: 较宽松，但大量请求建议分页
- WebSocket: 每个连接可订阅多个市场
- 超过限制返回 429，需指数退避重试

## 注意事项
- 所有金额使用 `Decimal` 类型，Polymarket 精度为 6 位 (USDC)
- API Key 和 Secret 必须存储在环境变量中，禁止硬编码
- CLOB API 需要先 approve USDC 到 Exchange 合约
- 测试阶段使用 Mumbai testnet，生产使用 Polygon mainnet
- WebSocket 消息需要心跳保活，否则 30s 超时断开
