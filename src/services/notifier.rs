use rust_decimal::Decimal;
use serde_json::json;

use crate::models::{CopyOrder, WhaleTradeEvent};

/// Telegram notification service. Failures are logged but never block the main flow.
#[derive(Debug, Clone)]
pub struct Notifier {
    http: reqwest::Client,
    bot_token: String,
    chat_id: String,
}

impl Notifier {
    pub fn new(bot_token: String, chat_id: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            bot_token,
            chat_id,
        }
    }

    /// Send a Telegram message. Failures are logged as warnings.
    pub async fn send(&self, message: &str) {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.bot_token
        );

        let body = json!({
            "chat_id": self.chat_id,
            "text": message,
            "parse_mode": "Markdown",
        });

        match self.http.post(&url).json(&body).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::warn!(
                        status = %resp.status(),
                        "Telegram sendMessage returned non-2xx"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to send Telegram notification");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn shorten_wallet(wallet: &str) -> String {
    if wallet.len() > 10 {
        format!("{}...{}", &wallet[..6], &wallet[wallet.len() - 4..])
    } else {
        wallet.to_string()
    }
}

fn side_cn(side: &str) -> &str {
    if side.eq_ignore_ascii_case("BUY") {
        "买入 YES 🟢 看多"
    } else {
        "卖出 YES 🔴 看空"
    }
}

fn market_label(market_question: Option<&str>, market_id: &str) -> String {
    match market_question {
        Some(q) if !q.is_empty() => q.to_string(),
        _ => {
            let end = 20.min(market_id.len());
            format!("{}...", &market_id[..end])
        }
    }
}

fn pnl_sign(v: Decimal) -> String {
    if v >= Decimal::ZERO {
        format!("+{}", v)
    } else {
        v.to_string()
    }
}

// ---------------------------------------------------------------------------
// 1. Copy signal — whale trade that passed all gates
// ---------------------------------------------------------------------------

pub fn format_copy_signal(
    event: &WhaleTradeEvent,
    win_rate: Decimal,
    kelly: Decimal,
    ev_copy: Decimal,
    market_question: Option<&str>,
) -> String {
    let market = market_label(market_question, &event.market_id);
    let wallet = shorten_wallet(&event.wallet);
    let side_string = event.side.to_string();
    let side = side_cn(&side_string);
    let wr = (win_rate * Decimal::ONE_HUNDRED).round_dp(1);

    format!(
        "🐋 *跟单信号*\n\n\
         📍 {market}\n\
         💰 {side}  {size} 份 @ ${price}\n\
         💵 ${notional} USDC\n\n\
         📊 巨鲸: `{wallet}`\n\
         ├ 胜率 {wr}% | 凯利 {kelly}\n\
         └ 调整后EV ${ev}",
        market = market,
        side = side,
        size = event.size,
        price = event.price,
        notional = event.notional.round_dp(2),
        wallet = wallet,
        wr = wr,
        kelly = kelly.round_dp(3),
        ev = ev_copy.round_dp(2),
    )
}

// ---------------------------------------------------------------------------
// 2. Basket consensus
// ---------------------------------------------------------------------------

pub fn format_consensus_alert(
    basket_name: &str,
    direction: &str,
    consensus_pct: Decimal,
    participating: i32,
    total: i32,
    market_id: &str,
    market_question: Option<&str>,
    price: Decimal,
    notional: Decimal,
) -> String {
    let market = market_label(market_question, market_id);
    let side = side_cn(direction);
    let pct = (consensus_pct * Decimal::from(100)).round_dp(0);

    format!(
        "🎯 *篮子共识达成*\n\n\
         📦 {basket} | 共识 {pct}% ({p}/{t})\n\
         📍 {market}\n\
         💰 {side}  当前价 ${price}\n\
         💵 触发交易: ${notional} USDC\n\n\
         {p}位高手48小时内一致{dir_cn}",
        basket = basket_name,
        pct = pct,
        p = participating,
        t = total,
        market = market,
        side = side,
        price = price,
        notional = notional.round_dp(2),
        dir_cn = if direction.eq_ignore_ascii_case("BUY") { "看多" } else { "看空" },
    )
}

// ---------------------------------------------------------------------------
// 3 & 4. Order result (filled / failed)
// ---------------------------------------------------------------------------

pub fn format_order_result(
    order: &CopyOrder,
    success: bool,
    error: Option<&str>,
    market_question: Option<&str>,
) -> String {
    let market = market_label(market_question, &order.market_id);

    if success {
        let fill = order.fill_price.unwrap_or(order.target_price);
        let side = side_cn(&order.side);
        format!(
            "✅ *订单成交*\n\n\
             📍 {market}\n\
             💰 {side}  {size} 份 @ ${fill}",
            market = market,
            side = side,
            size = order.size,
            fill = fill,
        )
    } else {
        let side = side_cn(&order.side);
        format!(
            "❌ *订单失败*\n\n\
             📍 {market}\n\
             💰 {side}  {size} 份\n\
             ⚠️ 原因: {err}",
            market = market,
            side = side,
            size = order.size,
            err = error.unwrap_or("unknown"),
        )
    }
}

// ---------------------------------------------------------------------------
// 5. Position exit (SL / TP)
// ---------------------------------------------------------------------------

pub fn format_position_exit(
    market_question: Option<&str>,
    market_id: &str,
    reason: &str,
    entry_price: Decimal,
    exit_price: Decimal,
    realized_pnl: Decimal,
    pnl_pct: Decimal,
) -> String {
    let market = market_label(market_question, market_id);
    let reason_cn = match reason {
        "stop_loss" => "止损",
        "take_profit" => "止盈",
        _ => reason,
    };

    format!(
        "📤 *持仓平仓*\n\n\
         📍 {market}\n\
         ⚡ 触发: {reason}\n\
         💰 入场 ${entry} → 出场 ${exit}\n\
         📊 盈亏: {pnl} USDC ({pnl_pct}%)",
        market = market,
        reason = reason_cn,
        entry = entry_price,
        exit = exit_price,
        pnl = pnl_sign(realized_pnl.round_dp(2)),
        pnl_pct = pnl_sign(pnl_pct.round_dp(2)),
    )
}

// ---------------------------------------------------------------------------
// 6. Market settled
// ---------------------------------------------------------------------------

pub fn format_market_settled(
    market_question: Option<&str>,
    market_id: &str,
    outcome: &str,
    positions_closed: usize,
    total_pnl: Decimal,
) -> String {
    let market = market_label(market_question, market_id);
    let outcome_cn = match outcome {
        "resolved_yes" | "resolved yes" => "Yes ✅",
        "resolved_no" | "resolved no" => "No ❎",
        other => other,
    };

    format!(
        "🏁 *市场结算*\n\n\
         📍 {market}\n\
         🎯 结果: {outcome}\n\
         📦 平仓: {count} 个持仓\n\
         📊 总盈亏: {pnl} USDC",
        market = market,
        outcome = outcome_cn,
        count = positions_closed,
        pnl = pnl_sign(total_pnl.round_dp(2)),
    )
}
