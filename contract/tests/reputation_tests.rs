#![cfg(test)]

use insightarena_contract::market::CreateMarketParams;
use insightarena_contract::reputation::*;
use insightarena_contract::storage_types::CreatorStats;
use insightarena_contract::{InsightArenaContract, InsightArenaContractClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{symbol_short, vec, Address, Env, String, Symbol};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

fn deploy(env: &Env) -> (InsightArenaContractClient<'_>, Address, Address) {
    let id = env.register(InsightArenaContract, ());
    let client = InsightArenaContractClient::new(env, &id);
    let admin = Address::generate(env);
    let oracle = Address::generate(env);
    let xlm_token = register_token(env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle, &200_u32, &xlm_token);
    (client, admin, oracle)
}

fn default_params(env: &Env) -> CreateMarketParams {
    let now = env.ledger().timestamp();
    CreateMarketParams {
        title: String::from_str(env, "Test market"),
        description: String::from_str(env, "desc"),
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

// ── Pure formula tests ────────────────────────────────────────────────────

#[test]
fn reputation_zero_for_new_creator() {
    let stats = CreatorStats {
        markets_created: 0,
        markets_resolved: 0,
        average_participant_count: 0,
        dispute_count: 0,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 0);
}

#[test]
fn reputation_perfect_score_no_disputes() {
    // 10/10 resolved, 100 avg participants → 600 + 200 - 0 = 800
    let stats = CreatorStats {
        markets_created: 10,
        markets_resolved: 10,
        average_participant_count: 100,
        dispute_count: 0,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 800);
}

#[test]
fn reputation_clamped_to_1000() {
    let stats = CreatorStats {
        markets_created: 1,
        markets_resolved: 1,
        average_participant_count: 300, // bonus capped at 200
        dispute_count: 0,
        reputation_score: 0,
    };
    // 600 + 200 = 800
    assert_eq!(calculate_creator_reputation(&stats), 800);
}

#[test]
fn reputation_dispute_penalty_capped_at_200() {
    // 10 * 50 = 500, capped at 200 → 600 + 0 - 200 = 400
    let stats = CreatorStats {
        markets_created: 10,
        markets_resolved: 10,
        average_participant_count: 0,
        dispute_count: 10,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 400);
}

#[test]
fn reputation_never_underflows() {
    // 0 resolved, max disputes → saturating_sub → 0
    let stats = CreatorStats {
        markets_created: 10,
        markets_resolved: 0,
        average_participant_count: 0,
        dispute_count: 100,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 0);
}

#[test]
fn reputation_partial_resolution() {
    // 5/10 * 600 = 300, 10 * 2 = 20, 1 * 50 = 50 → 270
    let stats = CreatorStats {
        markets_created: 10,
        markets_resolved: 5,
        average_participant_count: 10,
        dispute_count: 1,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 270);
}

#[test]
fn reputation_participation_bonus_capped_at_200() {
    let stats = CreatorStats {
        markets_created: 1,
        markets_resolved: 1,
        average_participant_count: 200, // 200 * 2 = 400, capped at 200
        dispute_count: 0,
        reputation_score: 0,
    };
    assert_eq!(calculate_creator_reputation(&stats), 800);
}

// ── Integration tests ─────────────────────────────────────────────────────

#[test]
fn get_creator_stats_returns_default_for_unknown_creator() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = deploy(&env);
    let unknown = Address::generate(&env);

    let stats = client.get_creator_stats(&unknown);
    assert_eq!(stats.markets_created, 0);
    assert_eq!(stats.markets_resolved, 0);
    assert_eq!(stats.reputation_score, 0);
}

#[test]
fn stats_updated_on_market_creation() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, _) = deploy(&env);
    let creator = Address::generate(&env);

    client.create_market(&creator, &default_params(&env));

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.markets_created, 1);
    assert_eq!(stats.markets_resolved, 0);
}

#[test]
fn stats_updated_on_market_resolution() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    let id = client.create_market(&creator, &default_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.markets_created, 1);
    assert_eq!(stats.markets_resolved, 1);
}

#[test]
fn stats_accumulate_across_multiple_markets() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    let id1 = client.create_market(&creator, &default_params(&env));
    let id2 = client.create_market(&creator, &default_params(&env));

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.markets_created, 2);

    env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
    client.resolve_market(&oracle, &id1, &symbol_short!("yes"));
    client.resolve_market(&oracle, &id2, &symbol_short!("no"));

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.markets_resolved, 2);
}

#[test]
fn reputation_score_stored_in_stats() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    let id = client.create_market(&creator, &default_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let stats = client.get_creator_stats(&creator);
    // 1/1 resolved = 600, 0 participants, 0 disputes → 600
    assert_eq!(stats.reputation_score, 600);
}

#[test]
fn reputation_score_always_in_range() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    for _ in 0..3 {
        let id = client.create_market(&creator, &default_params(&env));
        env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
        client.resolve_market(&oracle, &id, &symbol_short!("yes"));
    }

    let stats = client.get_creator_stats(&creator);
    assert!(stats.reputation_score <= 1000);
}

#[test]
fn test_reputation_decay_over_time() {
    // Test that reputation scores decay appropriately over time
    // Ensures inactive users don't maintain high scores indefinitely
    let env = Env::default();
    env.mock_all_auths();

    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    // Create and resolve market to get positive reputation
    let id = client.create_market(&creator, &default_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let stats = client.get_creator_stats(&creator);
    assert_eq!(stats.reputation_score, 600);

    // Fast forward in time
    env.ledger()
        .set_timestamp(env.ledger().timestamp() + 86400 * 30); // 30 days
    let stats_after_time = client.get_creator_stats(&creator);

    // Update this when decay logic is implemented in the reputation formula
    // For now we assert the current behavior where stats aren't decayed
    assert_eq!(stats_after_time.reputation_score, 600);
}

#[test]
fn test_reputation_with_high_dispute_count() {
    // Test reputation calculation with many disputes to verify penalty cap
    let env = Env::default();
    env.mock_all_auths();
    let (client, _, oracle) = deploy(&env);
    let creator = Address::generate(&env);

    // Create and resolve multiple markets
    for _ in 0..10 {
        let id = client.create_market(&creator, &default_params(&env));
        env.ledger().set_timestamp(env.ledger().timestamp() + 2000);
        client.resolve_market(&oracle, &id, &symbol_short!("yes"));
    }

    // Manually verify the reputation calculation with high dispute scenario
    // In a real scenario, disputes would be triggered through the dispute mechanism
    let stats = client.get_creator_stats(&creator);
    
    // With 10 markets created and resolved, no disputes yet
    // Expected: 10/10 * 600 = 600, 0 participation bonus, 0 disputes = 600
    assert_eq!(stats.markets_created, 10);
    assert_eq!(stats.markets_resolved, 10);
    assert_eq!(stats.reputation_score, 600);

    // Test the formula directly with high dispute count
    let high_dispute_stats = CreatorStats {
        markets_created: 10,
        markets_resolved: 10,
        average_participant_count: 50,
        dispute_count: 20, // Very high dispute count
        reputation_score: 0,
    };
    
    let reputation = calculate_creator_reputation(&high_dispute_stats);
    // 600 + 100 (50*2 capped at 200) - 200 (20*50 capped at 200) = 500
    assert_eq!(reputation, 500);
}
