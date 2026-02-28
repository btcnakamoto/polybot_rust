# Skill: 测试策略

## 概述
覆盖 Rust 后端和 React 前端的测试体系，确保核心交易逻辑的正确性和系统的稳定性。

## Rust 测试

### 依赖
```toml
[dev-dependencies]
tokio-test = "0.4"
wiremock = "0.6"
fake = { version = "2", features = ["derive"] }
assert_approx_eq = "1"
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres"] }  # 集成测试用
```

### 单元测试 — 核心逻辑
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_kelly_fraction() {
        // 60% 胜率, 1.5x 赔率
        let f = kelly_fraction(dec!(0.60), dec!(1.5));
        assert!(f > dec!(0.0) && f < dec!(1.0));
    }

    #[test]
    fn test_sharpe_ratio() {
        let returns = vec![dec!(0.05), dec!(0.03), dec!(-0.02), dec!(0.04), dec!(0.01)];
        let sharpe = sharpe_ratio(&returns);
        assert!(sharpe > dec!(0.0));
    }

    #[test]
    fn test_risk_check_rejects_oversized_position() {
        let limits = RiskLimits {
            max_position_pct: dec!(0.05),
            ..Default::default()
        };
        let portfolio = Portfolio { total_value: dec!(10000), ..Default::default() };
        let order = PendingOrder { size: dec!(1000), ..Default::default() };  // 10% > 5%
        assert!(check_risk(&order, &portfolio, &limits).is_err());
    }

    #[test]
    fn test_whale_classification() {
        // Market maker: 双边持仓
        let trades = vec![
            trade(Side::Buy, "YES", dec!(100)),
            trade(Side::Buy, "NO", dec!(100)),
        ];
        assert_eq!(classify_whale(&trades), Classification::MarketMaker);
    }
}
```

### 集成测试 — API & 数据库
```rust
// tests/api_tests.rs
use sqlx::PgPool;

#[sqlx::test(migrations = "./migrations")]
async fn test_add_whale(pool: PgPool) {
    let app = test_app(pool).await;
    let response = app.post("/api/whales")
        .json(&json!({ "address": "0xabc...", "category": "politics" }))
        .send()
        .await;
    assert_eq!(response.status(), 200);
}
```

### Mock 外部 API
```rust
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_fetch_whale_trades() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/trades"))
        .respond_with(ResponseTemplate::new(200).set_body_json(mock_trades()))
        .mount(&mock_server)
        .await;

    let client = DataApiClient::new(&mock_server.uri());
    let trades = client.get_trades("0xabc").await.unwrap();
    assert_eq!(trades.len(), 5);
}
```

### 回测框架
```rust
pub struct BacktestEngine {
    pub historical_trades: Vec<HistoricalTrade>,
    pub strategy: Box<dyn CopyStrategy>,
    pub initial_capital: Decimal,
}

impl BacktestEngine {
    pub fn run(&self) -> BacktestResult {
        let mut portfolio = Portfolio::new(self.initial_capital);
        for trade in &self.historical_trades {
            if let Some(order) = self.strategy.evaluate(trade, &portfolio) {
                portfolio.execute(order, trade);
            }
        }
        BacktestResult {
            final_value: portfolio.total_value,
            total_return: (portfolio.total_value - self.initial_capital) / self.initial_capital,
            max_drawdown: portfolio.max_drawdown(),
            sharpe: portfolio.sharpe_ratio(),
            total_trades: portfolio.trade_count,
            win_rate: portfolio.win_rate(),
        }
    }
}
```

## React 前端测试

### 依赖
```json
{
  "devDependencies": {
    "vitest": "^1",
    "@testing-library/react": "^14",
    "@testing-library/user-event": "^14",
    "msw": "^2"
  }
}
```

### 组件测试
```typescript
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { PortfolioSummary } from './PortfolioSummary';

describe('PortfolioSummary', () => {
  it('displays portfolio value', () => {
    render(<PortfolioSummary value={10000} pnl={500} todayPnl={50} />);
    expect(screen.getByText('$10,000.00')).toBeDefined();
  });

  it('shows green for positive PnL', () => {
    render(<PortfolioSummary value={10000} pnl={500} todayPnl={50} />);
    const pnlElement = screen.getByText('+$500.00');
    expect(pnlElement.className).toContain('text-emerald');
  });
});
```

### API Mock (MSW)
```typescript
import { setupServer } from 'msw/node';
import { http, HttpResponse } from 'msw';

export const handlers = [
  http.get('/api/whales', () => {
    return HttpResponse.json({ success: true, data: mockWhales });
  }),
  http.get('/api/dashboard/summary', () => {
    return HttpResponse.json({ success: true, data: mockSummary });
  }),
];

export const server = setupServer(...handlers);
```

## 测试优先级
1. **必须测试**: 跟单引擎核心逻辑、风控规则、仓位计算、钱包评分
2. **应该测试**: API 端点、数据库查询、WebSocket 消息处理
3. **可选测试**: UI 组件渲染、样式细节

## 运行命令
```bash
# Rust 测试
cargo test                    # 全部测试
cargo test --lib              # 仅单元测试
cargo test --test api_tests   # 仅集成测试

# React 测试
cd dashboard && npm test      # Vitest
```

## 注意事项
- 涉及金额的测试使用精确值断言，不用浮点近似
- 集成测试使用独立测试数据库，测试前自动迁移
- 外部 API 全部 mock，测试不依赖网络
- CI 中 Rust 和 React 测试并行运行
