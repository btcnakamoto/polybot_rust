use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

use crate::db::basket_repo::{self, BasketTradeVote};
use crate::models::{BasketCategory, WhaleBasket};

// ---------------------------------------------------------------------------
// Admission
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionResult {
    Accepted,
    Rejected(String),
}

/// Check whether a whale qualifies for basket admission.
///
/// Criteria:
/// - Win rate > 60%
/// - Active > 4 months (based on `months_active`)
/// - Not classified as bot or market_maker
/// - Average monthly trades < 100 (reject bots)
/// - Reject insider pattern: very few trades (< 5) but high win rate and short history
pub fn check_admission(
    win_rate: Decimal,
    classification: Option<&str>,
    months_active: i64,
    total_trades: i32,
    avg_monthly_trades: Decimal,
) -> AdmissionResult {
    // Win rate must exceed 60%
    if win_rate < Decimal::new(60, 2) {
        return AdmissionResult::Rejected("win rate below 60%".into());
    }

    // Must have been active for at least 4 months
    if months_active < 4 {
        return AdmissionResult::Rejected("history shorter than 4 months".into());
    }

    // Reject bot/market_maker classifications
    match classification {
        Some("bot") => return AdmissionResult::Rejected("classified as bot".into()),
        Some("market_maker") => {
            return AdmissionResult::Rejected("classified as market_maker".into())
        }
        _ => {}
    }

    // Reject high-frequency traders (likely bots even if not classified yet)
    if avg_monthly_trades > Decimal::from(100) {
        return AdmissionResult::Rejected("average monthly trades > 100 (bot pattern)".into());
    }

    // Reject suspicious insider pattern: very few trades + short history
    if total_trades < 5 && months_active < 6 {
        return AdmissionResult::Rejected(
            "suspected insider: too few trades with short history".into(),
        );
    }

    AdmissionResult::Accepted
}

// ---------------------------------------------------------------------------
// Consensus evaluation (pure function)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ConsensusCheck {
    pub reached: bool,
    pub direction: String,
    pub consensus_pct: Decimal,
    pub participating: i32,
    pub total: i32,
    pub reason: String,
}

/// Evaluate whether the votes in a basket reach consensus.
///
/// Pure function — no I/O.
///
/// Conditions:
/// 1. Same-direction vote ratio >= threshold (default 80%)
/// 2. Market price > 5¢ away from 0 or 1 (min_spread)
/// 3. At least 1 vote exists
pub fn evaluate_consensus(
    votes: &[BasketTradeVote],
    total_whales: i32,
    threshold: Decimal,
    market_price: Decimal,
    min_spread: Decimal,
) -> ConsensusCheck {
    let no_consensus = |reason: &str| ConsensusCheck {
        reached: false,
        direction: String::new(),
        consensus_pct: Decimal::ZERO,
        participating: votes.len() as i32,
        total: total_whales,
        reason: reason.to_string(),
    };

    if votes.is_empty() {
        return no_consensus("no votes in window");
    }

    // Check price distance from resolution (0 or 1)
    let dist_zero = market_price;
    let dist_one = Decimal::ONE - market_price;
    if dist_zero < min_spread || dist_one < min_spread {
        return no_consensus("market price too close to resolution");
    }

    // Count BUY vs SELL
    let buy_count = votes.iter().filter(|v| v.side.to_uppercase() == "BUY").count() as i32;
    let sell_count = votes.iter().filter(|v| v.side.to_uppercase() == "SELL").count() as i32;
    let total_votes = votes.len() as i32;

    let (majority_direction, majority_count) = if buy_count >= sell_count {
        ("BUY", buy_count)
    } else {
        ("SELL", sell_count)
    };

    let consensus_pct = if total_whales > 0 {
        Decimal::from(majority_count) / Decimal::from(total_whales)
    } else {
        Decimal::ZERO
    };

    if consensus_pct >= threshold {
        ConsensusCheck {
            reached: true,
            direction: majority_direction.to_string(),
            consensus_pct,
            participating: total_votes,
            total: total_whales,
            reason: format!(
                "consensus reached: {}/{} whales vote {}",
                majority_count, total_whales, majority_direction
            ),
        }
    } else {
        ConsensusCheck {
            reached: false,
            direction: majority_direction.to_string(),
            consensus_pct,
            participating: total_votes,
            total: total_whales,
            reason: format!(
                "consensus not reached: {:.1}% < {:.1}% threshold",
                consensus_pct * Decimal::ONE_HUNDRED,
                threshold * Decimal::ONE_HUNDRED,
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Market category inference
// ---------------------------------------------------------------------------

/// Infer a basket category from a market question using keyword matching.
pub fn infer_market_category(question: &str) -> Option<BasketCategory> {
    let q = question.to_lowercase();

    let politics_keywords = [
        "president", "election", "trump", "biden", "congress", "senate",
        "governor", "democrat", "republican", "vote", "ballot", "political",
        "party", "legislation", "minister", "parliament", "nato",
    ];
    let crypto_keywords = [
        "bitcoin", "btc", "ethereum", "eth", "crypto", "token", "blockchain",
        "solana", "sol", "dogecoin", "doge", "defi", "nft", "altcoin",
    ];
    let sports_keywords = [
        "nba", "nfl", "mlb", "nhl", "fifa", "world cup", "championship",
        "super bowl", "premier league", "playoffs", "mvp", "touchdown",
        "slam dunk", "goal", "match", "tennis", "ufc", "boxing",
    ];

    if politics_keywords.iter().any(|kw| q.contains(kw)) {
        return Some(BasketCategory::Politics);
    }
    if crypto_keywords.iter().any(|kw| q.contains(kw)) {
        return Some(BasketCategory::Crypto);
    }
    if sports_keywords.iter().any(|kw| q.contains(kw)) {
        return Some(BasketCategory::Sports);
    }

    None
}

// ---------------------------------------------------------------------------
// Auto-assign whale to matching baskets
// ---------------------------------------------------------------------------

/// Automatically add a whale to active baskets that match the given category
/// and have room (count < max_wallets). Returns names of baskets assigned to.
pub async fn auto_assign_to_baskets(
    pool: &PgPool,
    whale_id: Uuid,
    category: &str,
) -> anyhow::Result<Vec<String>> {
    let baskets = basket_repo::get_active_baskets(pool).await?;
    let mut assigned = Vec::new();

    for basket in &baskets {
        if basket.category != category {
            continue;
        }

        let count = basket_repo::count_basket_whales(pool, basket.id).await?;
        if count >= basket.max_wallets as i64 {
            tracing::debug!(
                basket = %basket.name,
                count = count,
                max = basket.max_wallets,
                "Basket full — skipping auto-assign"
            );
            continue;
        }

        basket_repo::add_whale_to_basket(pool, basket.id, whale_id).await?;
        assigned.push(basket.name.clone());
    }

    Ok(assigned)
}

// ---------------------------------------------------------------------------
// Async pipeline — ties DB queries to pure evaluation
// ---------------------------------------------------------------------------

/// Check basket consensus for a specific market, using DB queries.
pub async fn check_basket_consensus(
    pool: &PgPool,
    basket: &WhaleBasket,
    market_id: &str,
    market_price: Decimal,
) -> anyhow::Result<ConsensusCheck> {
    let since = Utc::now() - Duration::hours(basket.time_window_hours as i64);

    let votes =
        basket_repo::get_basket_trades_in_window(pool, basket.id, market_id, since).await?;

    let total_whales = basket_repo::count_basket_whales(pool, basket.id).await? as i32;

    let min_spread = Decimal::new(5, 2); // 0.05 = 5¢

    let check = evaluate_consensus(
        &votes,
        total_whales,
        basket.consensus_threshold,
        market_price,
        min_spread,
    );

    Ok(check)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_vote(whale_id: Uuid, side: &str) -> BasketTradeVote {
        BasketTradeVote {
            whale_id,
            side: side.to_string(),
            traded_at: Utc::now(),
        }
    }

    // --- Admission tests ---

    #[test]
    fn test_admission_accepted() {
        let result = check_admission(
            Decimal::new(70, 2), // 0.70 win rate
            Some("informed"),
            6,   // 6 months
            50,  // 50 trades
            Decimal::from(10),
        );
        assert_eq!(result, AdmissionResult::Accepted);
    }

    #[test]
    fn test_admission_low_win_rate() {
        let result = check_admission(
            Decimal::new(50, 2), // 0.50 — too low
            Some("informed"),
            6,
            50,
            Decimal::from(10),
        );
        assert!(matches!(result, AdmissionResult::Rejected(ref r) if r.contains("win rate")));
    }

    #[test]
    fn test_admission_short_history() {
        let result = check_admission(
            Decimal::new(70, 2),
            Some("informed"),
            2, // only 2 months
            50,
            Decimal::from(10),
        );
        assert!(matches!(result, AdmissionResult::Rejected(ref r) if r.contains("4 months")));
    }

    #[test]
    fn test_admission_bot_frequency() {
        let result = check_admission(
            Decimal::new(70, 2),
            Some("informed"),
            6,
            500,
            Decimal::from(150), // 150 trades/month
        );
        assert!(
            matches!(result, AdmissionResult::Rejected(ref r) if r.contains("bot pattern"))
        );
    }

    #[test]
    fn test_admission_classification_rejected() {
        let result = check_admission(
            Decimal::new(70, 2),
            Some("bot"),
            6,
            50,
            Decimal::from(10),
        );
        assert!(matches!(result, AdmissionResult::Rejected(ref r) if r.contains("bot")));

        let result2 = check_admission(
            Decimal::new(70, 2),
            Some("market_maker"),
            6,
            50,
            Decimal::from(10),
        );
        assert!(
            matches!(result2, AdmissionResult::Rejected(ref r) if r.contains("market_maker"))
        );
    }

    #[test]
    fn test_admission_insider_pattern() {
        let result = check_admission(
            Decimal::new(90, 2), // suspiciously high
            Some("informed"),
            5, // meets 4-month minimum, but still short
            3, // very few trades
            Decimal::from(1),
        );
        assert!(matches!(result, AdmissionResult::Rejected(ref r) if r.contains("insider")));
    }

    // --- Consensus tests ---

    #[test]
    fn test_consensus_all_buy() {
        let votes: Vec<BasketTradeVote> = (0..5)
            .map(|_| make_vote(Uuid::new_v4(), "BUY"))
            .collect();

        let check = evaluate_consensus(
            &votes,
            5,
            Decimal::new(80, 2),
            Decimal::new(50, 2), // 0.50 price
            Decimal::new(5, 2),  // 0.05 min spread
        );

        assert!(check.reached);
        assert_eq!(check.direction, "BUY");
        assert_eq!(check.consensus_pct, Decimal::ONE);
    }

    #[test]
    fn test_consensus_mixed_no_reach() {
        let mut votes = Vec::new();
        for _ in 0..3 {
            votes.push(make_vote(Uuid::new_v4(), "BUY"));
        }
        for _ in 0..2 {
            votes.push(make_vote(Uuid::new_v4(), "SELL"));
        }

        let check = evaluate_consensus(
            &votes,
            5,
            Decimal::new(80, 2),
            Decimal::new(50, 2),
            Decimal::new(5, 2),
        );

        assert!(!check.reached);
        // 3/5 = 0.60 < 0.80
        assert_eq!(check.consensus_pct, Decimal::new(6, 1));
    }

    #[test]
    fn test_consensus_exact_80_boundary() {
        // 4 out of 5 = exactly 80%
        let mut votes = Vec::new();
        for _ in 0..4 {
            votes.push(make_vote(Uuid::new_v4(), "BUY"));
        }
        votes.push(make_vote(Uuid::new_v4(), "SELL"));

        let check = evaluate_consensus(
            &votes,
            5,
            Decimal::new(80, 2), // threshold = 0.80
            Decimal::new(50, 2),
            Decimal::new(5, 2),
        );

        assert!(check.reached);
        assert_eq!(check.consensus_pct, Decimal::new(8, 1)); // 0.8
    }

    #[test]
    fn test_consensus_price_too_close() {
        let votes: Vec<BasketTradeVote> = (0..5)
            .map(|_| make_vote(Uuid::new_v4(), "BUY"))
            .collect();

        // Price at 0.97 → only 0.03 from 1.0, below 0.05 min spread
        let check = evaluate_consensus(
            &votes,
            5,
            Decimal::new(80, 2),
            Decimal::new(97, 2), // 0.97
            Decimal::new(5, 2),
        );

        assert!(!check.reached);
        assert!(check.reason.contains("too close"));
    }

    #[test]
    fn test_consensus_empty_votes() {
        let check = evaluate_consensus(
            &[],
            5,
            Decimal::new(80, 2),
            Decimal::new(50, 2),
            Decimal::new(5, 2),
        );

        assert!(!check.reached);
        assert!(check.reason.contains("no votes"));
    }

    // --- Category inference tests ---

    #[test]
    fn test_infer_category_politics() {
        assert_eq!(
            infer_market_category("Will Trump win the 2024 election?"),
            Some(BasketCategory::Politics)
        );
        assert_eq!(
            infer_market_category("Will the Senate pass the bill?"),
            Some(BasketCategory::Politics)
        );
    }

    #[test]
    fn test_infer_category_crypto() {
        assert_eq!(
            infer_market_category("Will Bitcoin reach $100k by end of year?"),
            Some(BasketCategory::Crypto)
        );
        assert_eq!(
            infer_market_category("Will ETH flip BTC in market cap?"),
            Some(BasketCategory::Crypto)
        );
    }

    #[test]
    fn test_infer_category_sports() {
        assert_eq!(
            infer_market_category("Who will win the Super Bowl?"),
            Some(BasketCategory::Sports)
        );
        assert_eq!(
            infer_market_category("Will the NBA MVP be from the West?"),
            Some(BasketCategory::Sports)
        );
    }

    #[test]
    fn test_infer_category_unknown() {
        assert_eq!(
            infer_market_category("Will it rain in Paris tomorrow?"),
            None
        );
        assert_eq!(
            infer_market_category("What is the meaning of life?"),
            None
        );
    }

    #[test]
    fn test_consensus_sell_direction() {
        let votes: Vec<BasketTradeVote> = (0..5)
            .map(|_| make_vote(Uuid::new_v4(), "SELL"))
            .collect();

        let check = evaluate_consensus(
            &votes,
            5,
            Decimal::new(80, 2),
            Decimal::new(50, 2),
            Decimal::new(5, 2),
        );

        assert!(check.reached);
        assert_eq!(check.direction, "SELL");
    }
}
