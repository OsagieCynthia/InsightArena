use insightarena_contract::governance::ProposalType;
use insightarena_contract::storage_types::DataKey;
use insightarena_contract::{InsightArenaContract, InsightArenaContractClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Symbol, Vec};

// ── Helpers ────────────────────────────────────────────────────────────────────

fn register_token(env: &Env) -> Address {
    let token_admin = Address::generate(env);
    env.register_stellar_asset_contract_v2(token_admin)
        .address()
}

fn deploy(env: &Env) -> (InsightArenaContractClient<'_>, Address) {
    let id = env.register(InsightArenaContract, ());
    let client = InsightArenaContractClient::new(env, &id);
    let admin = Address::generate(env);
    let oracle = Address::generate(env);
    let xlm_token = register_token(env);
    env.mock_all_auths();
    client.initialize(&admin, &oracle, &200_u32, &xlm_token);
    (client, admin)
}

fn create_fee_proposal(
    client: &InsightArenaContractClient<'_>,
    proposer: &Address,
    duration: u64,
) -> u32 {
    client.create_proposal(proposer, &ProposalType::UpdateProtocolFee(300), &duration)
}

/// Seed `n` addresses into the UserList so quorum can be met.
fn seed_users(env: &Env, client: &InsightArenaContractClient<'_>, n: u32) -> Vec<Address> {
    let mut users: Vec<Address> = Vec::new(env);
    for _ in 0..n {
        users.push_back(Address::generate(env));
    }
    env.as_contract(&client.address, || {
        env.storage().persistent().set(&DataKey::UserList, &users);
    });
    users
}

/// Create a proposal, cast enough votes to meet quorum, then advance time past voting_end.
fn pass_proposal(
    env: &Env,
    client: &InsightArenaContractClient<'_>,
    proposal_type: &ProposalType,
    voters: &Vec<Address>,
) -> u32 {
    let duration = 3_600_u64;
    let proposer = Address::generate(env);
    let id = client.create_proposal(&proposer, proposal_type, &duration);
    for voter in voters.iter() {
        client.vote(&voter, &id, &true);
    }
    env.ledger().with_mut(|l| l.timestamp += duration + 1);
    id
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[test]
fn test_list_proposals_empty_before_any_proposals() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);

    // No proposals created yet — every pagination call must return an empty list.
    assert_eq!(client.list_proposals(&1_u32, &10_u32).len(), 0);
    assert_eq!(client.list_proposals(&0_u32, &10_u32).len(), 0);
}

#[test]
fn test_list_proposals_returns_all_proposals() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let proposer = Address::generate(&env);

    let id1 = create_fee_proposal(&client, &proposer, 3600);
    let id2 = create_fee_proposal(&client, &proposer, 7200);
    let id3 = create_fee_proposal(&client, &proposer, 10_800);

    let list = client.list_proposals(&1_u32, &10_u32);

    assert_eq!(list.len(), 3);
    assert_eq!(list.get(0).unwrap().proposal_id, id1);
    assert_eq!(list.get(1).unwrap().proposal_id, id2);
    assert_eq!(list.get(2).unwrap().proposal_id, id3);
}

#[test]
fn test_list_proposals_pagination_works() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let proposer = Address::generate(&env);

    for _ in 0..5 {
        create_fee_proposal(&client, &proposer, 3600);
    }

    // Page 1: IDs 1–2
    let page1 = client.list_proposals(&1_u32, &2_u32);
    assert_eq!(page1.len(), 2);
    assert_eq!(page1.get(0).unwrap().proposal_id, 1);
    assert_eq!(page1.get(1).unwrap().proposal_id, 2);

    // Page 2: IDs 3–4
    let page2 = client.list_proposals(&3_u32, &2_u32);
    assert_eq!(page2.len(), 2);
    assert_eq!(page2.get(0).unwrap().proposal_id, 3);
    assert_eq!(page2.get(1).unwrap().proposal_id, 4);

    // Page 3: ID 5 only
    let page3 = client.list_proposals(&5_u32, &2_u32);
    assert_eq!(page3.len(), 1);
    assert_eq!(page3.get(0).unwrap().proposal_id, 5);

    // Out-of-bounds start returns empty
    assert_eq!(client.list_proposals(&6_u32, &10_u32).len(), 0);

    // Limit capped at 50
    let big = client.list_proposals(&1_u32, &100_u32);
    assert_eq!(big.len(), 5); // only 5 proposals exist
}

// ── execute_proposal Tests ─────────────────────────────────────────────────────

#[test]
fn test_execute_proposal_updates_protocol_fee() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let voters = seed_users(&env, &client, 5);

    let id = pass_proposal(&env, &client, &ProposalType::UpdateProtocolFee(500), &voters);

    let executor = Address::generate(&env);
    client.execute_proposal(&executor, &id);

    let cfg = client.get_config().unwrap();
    assert_eq!(cfg.protocol_fee_bps, 500);
}

#[test]
fn test_execute_proposal_updates_oracle() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let voters = seed_users(&env, &client, 5);

    let new_oracle = Address::generate(&env);
    let id = pass_proposal(
        &env,
        &client,
        &ProposalType::UpdateOracle(new_oracle.clone()),
        &voters,
    );

    let executor = Address::generate(&env);
    client.execute_proposal(&executor, &id);

    let cfg = client.get_config().unwrap();
    assert_eq!(cfg.oracle_address, new_oracle);
}

#[test]
fn test_execute_proposal_updates_min_stake() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let voters = seed_users(&env, &client, 5);

    let new_min = 50_000_000_i128; // 5 XLM in stroops
    let id = pass_proposal(
        &env,
        &client,
        &ProposalType::UpdateMinStake(new_min),
        &voters,
    );

    let executor = Address::generate(&env);
    client.execute_proposal(&executor, &id);

    let cfg = client.get_config().unwrap();
    assert_eq!(cfg.min_stake_xlm, new_min);
}

#[test]
fn test_execute_proposal_adds_category() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let voters = seed_users(&env, &client, 5);

    let new_cat = Symbol::new(&env, "Gaming");
    let id = pass_proposal(
        &env,
        &client,
        &ProposalType::AddSupportedCategory(new_cat.clone()),
        &voters,
    );

    let executor = Address::generate(&env);
    client.execute_proposal(&executor, &id);

    let categories = client.list_categories();
    assert!(categories.contains(new_cat));
}

#[test]
fn test_execute_proposal_fails_without_quorum() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);

    // Seed 10 users but cast 0 votes — quorum (1) not met
    seed_users(&env, &client, 10);

    let duration = 3_600_u64;
    let proposer = Address::generate(&env);
    let id = client.create_proposal(&proposer, &ProposalType::UpdateProtocolFee(400), &duration);

    env.ledger().with_mut(|l| l.timestamp += duration + 1);

    let executor = Address::generate(&env);
    let result = client.try_execute_proposal(&executor, &id);
    assert!(result.is_err());
}

#[test]
fn test_execute_proposal_fails_before_voting_ends() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let voters = seed_users(&env, &client, 5);

    let duration = 3_600_u64;
    let proposer = Address::generate(&env);
    let id = client.create_proposal(&proposer, &ProposalType::UpdateProtocolFee(400), &duration);
    for voter in voters.iter() {
        client.vote(&voter, &id, &true);
    }

    // Do NOT advance time — voting period still active
    let executor = Address::generate(&env);
    let result = client.try_execute_proposal(&executor, &id);
    assert!(result.is_err());
}

// ── get_proposal Tests ─────────────────────────────────────────────────────────

#[test]
fn test_get_proposal_returns_correct_fields() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let proposer = Address::generate(&env);

    let id = client.create_proposal(&proposer, &ProposalType::UpdateProtocolFee(400), &3_600_u64);
    let proposal = client.get_proposal(&id).unwrap();

    assert_eq!(proposal.proposal_id, id);
    assert_eq!(proposal.proposer, proposer);
    assert_eq!(proposal.votes_for, 0);
    assert_eq!(proposal.votes_against, 0);
    assert!(!proposal.executed);
}

#[test]
fn test_get_proposal_fails_for_unknown_id() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);

    let result = client.try_get_proposal(&999_u32);
    assert!(result.is_err());
}

#[test]
fn test_get_proposal_reflects_votes_after_voting() {
    let env = Env::default();
    env.mock_all_auths();
    let (client, _) = deploy(&env);
    let proposer = Address::generate(&env);

    let id = client.create_proposal(&proposer, &ProposalType::UpdateProtocolFee(400), &3_600_u64);

    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    client.vote(&voter_a, &id, &true);
    client.vote(&voter_b, &id, &false);

    let proposal = client.get_proposal(&id).unwrap();
    assert_eq!(proposal.votes_for, 1);
    assert_eq!(proposal.votes_against, 1);
}
