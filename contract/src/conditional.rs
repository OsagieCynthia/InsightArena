#![allow(unused_imports)]
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::config;
use crate::errors::InsightArenaError;
use crate::market;
use crate::storage_types::{ConditionalMarket, DataKey, Market};
use crate::ttl;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum depth of conditional market chains
pub const MAX_CONDITIONAL_DEPTH: u32 = 5;

/// Maximum number of conditional markets per parent
pub const MAX_CONDITIONALS_PER_PARENT: u32 = 50;

// ── Conditional Market Creation ──────────────────────────────────────────────

use crate::market::CreateMarketParams;

pub fn create_conditional_market(
    env: &Env,
    creator: Address,
    parent_market_id: u64,
    required_outcome: Symbol,
    params: CreateMarketParams,
) -> Result<u64, InsightArenaError> {
    let parent_market: Market = env
        .storage()
        .persistent()
        .get(&DataKey::Market(parent_market_id))
        .ok_or(InsightArenaError::MarketNotFound)?;

    if parent_market.is_resolved {
        return Err(InsightArenaError::MarketExpired);
    }

    if !parent_market.outcome_options.contains(required_outcome.clone()) {
        return Err(InsightArenaError::InvalidOutcome);
    }

    let mut depth = 1;
    if let Some(parent_cond) = env
        .storage()
        .persistent()
        .get::<_, ConditionalMarket>(&DataKey::ConditionalMarket(parent_market_id))
    {
        depth = parent_cond.conditional_depth + 1;
        if depth > MAX_CONDITIONAL_DEPTH {
            return Err(InsightArenaError::ConditionalDepthExceeded);
        }
    }

    let market_id = crate::market::create_market(env, creator, params)?;

    let conditional_market = ConditionalMarket::new(
        market_id,
        parent_market_id,
        required_outcome,
        depth,
        env.ledger().timestamp(),
    );
    env.storage()
        .persistent()
        .set(&DataKey::ConditionalMarket(market_id), &conditional_market);

    let children_key = DataKey::ConditionalChildren(parent_market_id);
    let mut children: Vec<u64> = env
        .storage()
        .persistent()
        .get(&children_key)
        .unwrap_or_else(|| Vec::new(env));
    children.push_back(market_id);
    env.storage().persistent().set(&children_key, &children);

    env.storage().persistent().set(&DataKey::ConditionalParent(market_id), &parent_market_id);

    Ok(market_id)
}

// TODO: validate_conditional_params
// TODO: link_conditional_market

// ── Activation Logic ──────────────────────────────────────────────────────────

// TODO: check_conditional_activation
// TODO: activate_conditional_market
// TODO: deactivate_conditional_market

// ── Query Functions ───────────────────────────────────────────────────────────

// TODO: get_conditional_markets
// TODO: get_parent_market
// TODO: get_conditional_chain
// TODO: is_conditional_market

// ── Helper Functions ──────────────────────────────────────────────────────────

// TODO: calculate_conditional_depth
// TODO: validate_no_circular_dependency
