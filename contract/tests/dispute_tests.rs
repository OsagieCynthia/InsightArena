use insightarena_contract::{
    CreateMarketParams, InsightArenaContract, InsightArenaContractClient, InsightArenaError,
};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::token::{Client as TokenClient, StellarAssetClient};
use soroban_sdk::{symbol_short, vec, Address, Env, String, Symbol};

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin).address()
}

fn deploy(env: &Env) -> (InsightArenaContractClient<'_>, Address, Address, Address) {
    let id = env.register(InsightArenaContract, ());
    let client = InsightArenaContractClient::new(env, &id);
    let admin = Address::generate(env);
    let oracle = Address::generate(env);
    let xlm_token = register_token(env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle, &200_u32, &xlm_token);
    (client, admin, oracle, xlm_token)
}

fn market_params(env: &Env) -> CreateMarketParams {
    let now = env.ledger().timestamp();
    CreateMarketParams {
        title: String::from_str(env, "Dispute test market"),
        description: String::from_str(env, "For get_dispute tests"),
        category: Symbol::new(env, "Sports"),
        outcomes: vec![env, symbol_short!("yes"), symbol_short!("no")],
        end_time: now + 10,
        resolution_time: now + 20,
        dispute_window: 86_400,
        creator_fee_bps: 100,
        min_stake: 10_000_000,
        max_stake: 100_000_000,
        is_public: true,
    }
}

#[test]
fn test_get_dispute_returns_correct_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, oracle, xlm_token) = deploy(&env);
    let creator = Address::generate(&env);
    let disputer = Address::generate(&env);

    let id = client.create_market(&creator, &market_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 20);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let bond = 15_000_000_i128;
    StellarAssetClient::new(&env, &xlm_token).mint(&disputer, &bond);
    TokenClient::new(&env, &xlm_token).approve(&disputer, &client.address, &bond, &9999);

    let filed_at = env.ledger().timestamp();
    client.raise_dispute(&disputer, &id, &bond);

    let dispute = client.get_dispute(&id);
    assert_eq!(dispute.disputer, disputer);
    assert_eq!(dispute.bond, bond);
    assert_eq!(dispute.filed_at, filed_at);
}

#[test]
fn test_get_dispute_fails_when_no_dispute() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _admin, oracle, _xlm_token) = deploy(&env);
    let creator = Address::generate(&env);

    let id = client.create_market(&creator, &market_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 20);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let result = client.try_get_dispute(&id);
    assert!(matches!(result, Err(Ok(InsightArenaError::DisputeNotFound))));
}

#[test]
fn test_get_dispute_fails_after_resolution() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, admin, oracle, xlm_token) = deploy(&env);
    let creator = Address::generate(&env);
    let disputer = Address::generate(&env);

    let id = client.create_market(&creator, &market_params(&env));
    env.ledger().set_timestamp(env.ledger().timestamp() + 20);
    client.resolve_market(&oracle, &id, &symbol_short!("yes"));

    let bond = 12_000_000_i128;
    StellarAssetClient::new(&env, &xlm_token).mint(&disputer, &bond);
    TokenClient::new(&env, &xlm_token).approve(&disputer, &client.address, &bond, &9999);
    client.raise_dispute(&disputer, &id, &bond);

    // Reject the dispute — this removes it from storage
    client.resolve_dispute(&admin, &id, &false);

    let result = client.try_get_dispute(&id);
    assert!(matches!(result, Err(Ok(InsightArenaError::DisputeNotFound))));
}
