#![cfg(test)]

use insightarena_contract::market::CreateMarketParams;
use insightarena_contract::storage_types::{DataKey, Market};
use insightarena_contract::{InsightArenaContract, InsightArenaContractClient, InsightArenaError};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, vec, Address, Env, String, Symbol};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

fn deploy(env: &Env) -> InsightArenaContractClient<'_> {
    let id = env.register(InsightArenaContract, ());
    let client = InsightArenaContractClient::new(env, &id);
    let admin = Address::generate(env);
    let oracle = Address::generate(env);
    let xlm_token = register_token(env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle, &200_u32, &xlm_token);
    client
}

fn default_params(env: &Env) -> CreateMarketParams {
    let now = env.ledger().timestamp();
    CreateMarketParams {
        title: String::from_str(env, "Will it rain?"),
        description: String::from_str(env, "Daily weather market"),
        category: Symbol::new(env, "Sports"),
        outcomes: vec![env, symbol_short!("yes"), symbol_short!("no")],
        end_time: now + 1000,
        resolution_time: now + 2000,
        dispute_window: 86_400,
        creator_fee_bps: 100,
        min_stake: 10_000_000,
        max_stake: 100_000_000,
        is_public: true,
    }
}

#[test]
fn test_create_conditional_market_invalid_parent_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let creator = Address::generate(&env);

    let required_outcome = symbol_short!("yes");

    let result = client.try_create_conditional_market(
        &creator,
        &999_u64, // Invalid parent
        &required_outcome,
        &default_params(&env),
    );

    assert!(matches!(result, Err(Ok(InsightArenaError::MarketNotFound))));
}

#[test]
fn test_create_conditional_market_resolved_parent_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let creator = Address::generate(&env);

    // Create parent market
    let parent_id = client.create_market(&creator, &default_params(&env));

    // Force parent market to be resolved
    let contract_id = client.address.clone();
    let mut market: Market = env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .get(&DataKey::Market(parent_id))
            .unwrap()
    });
    market.is_resolved = true;
    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .set(&DataKey::Market(parent_id), &market);
    });

    let required_outcome = symbol_short!("yes");

    let result = client.try_create_conditional_market(
        &creator,
        &parent_id,
        &required_outcome,
        &default_params(&env),
    );

    assert!(matches!(result, Err(Ok(InsightArenaError::MarketExpired))));
}

#[test]
fn test_create_conditional_market_invalid_outcome_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let creator = Address::generate(&env);

    // Create parent market (outcomes are yes, no)
    let parent_id = client.create_market(&creator, &default_params(&env));

    // invalid outcome
    let required_outcome = symbol_short!("maybe");

    let result = client.try_create_conditional_market(
        &creator,
        &parent_id,
        &required_outcome,
        &default_params(&env),
    );

    assert!(matches!(result, Err(Ok(InsightArenaError::InvalidOutcome))));
}

#[test]
fn test_create_conditional_market_exceeds_depth_fails() {
    let env = Env::default();
    env.mock_all_auths();
    let client = deploy(&env);
    let creator = Address::generate(&env);

    let required_outcome = symbol_short!("yes");

    // Create root parent
    let mut parent_id = client.create_market(&creator, &default_params(&env));

    // Depth limits MAX_CONDITIONAL_DEPTH = 5
    // creating 5 nested conditionals should be okay (depths 2, 3, 4, 5, wait, MAX=5, so 4 creations from root)
    // Root is depth 0. The first conditional is depth 1.
    // Wait, the logic sets conditional to `depth = parent_cond.conditional_depth + 1`. If no parent_cond, depth = 1.
    // So root is not a conditional market. The first conditional is depth 1.
    // So 5 nested:
    for _ in 0..5 {
        parent_id = client.create_conditional_market(
            &creator,
            &parent_id,
            &required_outcome,
            &default_params(&env),
        );
    }

    // Now depth is 5. Another creation should fail with ConditionalDepthExceeded
    let result = client.try_create_conditional_market(
        &creator,
        &parent_id,
        &required_outcome,
        &default_params(&env),
    );

    assert!(matches!(
        result,
        Err(Ok(InsightArenaError::ConditionalDepthExceeded))
    ));
}
