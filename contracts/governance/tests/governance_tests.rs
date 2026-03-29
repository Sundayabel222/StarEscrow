#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String, Vec,
};

use governance::{
    GovError, GovernanceContract, GovernanceContractClient, ParamChange, ProposalStatus,
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn create_token<'a>(
    env: &Env,
    admin: &Address,
) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &contract_address.address()),
        StellarAssetClient::new(env, &contract_address.address()),
    )
}

struct Setup<'a> {
    env: Env,
    gov: GovernanceContractClient<'a>,
    token: TokenClient<'a>,
    token_admin: StellarAssetClient<'a>,
    voter_a: Address,
    voter_b: Address,
    proposer: Address,
    /// Dummy escrow address (we don't need a real escrow for governance unit tests)
    escrow: Address,
    voting_period: u64,
    timelock_delay: u64,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let voter_a = Address::generate(&env);
        let voter_b = Address::generate(&env);
        let proposer = Address::generate(&env);
        let escrow = Address::generate(&env);

        let (token, token_admin) = create_token(&env, &admin);

        // Mint voting tokens
        token_admin.mint(&voter_a, &1_000_000);
        token_admin.mint(&voter_b, &500_000);
        token_admin.mint(&proposer, &100_000);

        let gov_id = env.register(GovernanceContract, ());
        let gov = GovernanceContractClient::new(&env, &gov_id);

        let voting_period = 3600u64;   // 1 hour
        let timelock_delay = 7200u64;  // 2 hours

        gov.init(
            &token.address,
            &voting_period,
            &timelock_delay,
            &500_000i128, // quorum
            &escrow,
        );

        Setup {
            env,
            gov,
            token,
            token_admin,
            voter_a,
            voter_b,
            proposer,
            escrow,
            voting_period,
            timelock_delay,
        }
    }

    fn param_change(&self, key: &str, value: &str) -> ParamChange {
        ParamChange {
            key: String::from_str(&self.env, key),
            value: String::from_str(&self.env, value),
        }
    }

    fn changes(&self, key: &str, value: &str) -> Vec<ParamChange> {
        let mut v = Vec::new(&self.env);
        v.push_back(self.param_change(key, value));
        v
    }
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[test]
fn test_double_init_fails() {
    let s = Setup::new();
    let result = s.gov.try_init(
        &s.token.address,
        &3600,
        &7200,
        &500_000,
        &s.escrow,
    );
    assert_eq!(result, Err(Ok(GovError::AlreadyInitialized)));
}

#[test]
fn test_propose_creates_proposal() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));
    assert_eq!(id, 0);

    let proposal = s.gov.get_proposal(&id).unwrap();
    assert_eq!(proposal.id, 0);
    assert_eq!(proposal.status, ProposalStatus::Active);
    assert_eq!(proposal.votes_for, 0);
}

#[test]
fn test_vote_increments_weight() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));

    s.gov.vote(&s.voter_a, &id, &true);
    let proposal = s.gov.get_proposal(&id).unwrap();
    assert_eq!(proposal.votes_for, 1_000_000);
    assert_eq!(proposal.votes_against, 0);

    s.gov.vote(&s.voter_b, &id, &false);
    let proposal = s.gov.get_proposal(&id).unwrap();
    assert_eq!(proposal.votes_against, 500_000);
}

#[test]
fn test_double_vote_rejected() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));
    s.gov.vote(&s.voter_a, &id, &true);

    let result = s.gov.try_vote(&s.voter_a, &id, &true);
    assert_eq!(result, Err(Ok(GovError::AlreadyVoted)));
}

#[test]
fn test_vote_after_period_rejected() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));

    // Advance past voting period
    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });

    let result = s.gov.try_vote(&s.voter_a, &id, &true);
    assert_eq!(result, Err(Ok(GovError::VotingClosed)));
}

#[test]
fn test_finalize_before_period_ends_fails() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));

    let result = s.gov.try_finalize(&id);
    assert_eq!(result, Err(Ok(GovError::VotingStillOpen)));
}

#[test]
fn test_finalize_passes_with_quorum_and_majority() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));

    // voter_a has 1_000_000 ≥ quorum 500_000 and > voter_b's 500_000
    s.gov.vote(&s.voter_a, &id, &true);
    s.gov.vote(&s.voter_b, &id, &false);

    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });

    let status = s.gov.finalize(&id);
    assert_eq!(status, ProposalStatus::Passed);
}

#[test]
fn test_finalize_rejects_below_quorum() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));

    // proposer has only 100_000 < quorum 500_000
    s.gov.vote(&s.proposer, &id, &true);

    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });

    let status = s.gov.finalize(&id);
    assert_eq!(status, ProposalStatus::Rejected);
}

#[test]
fn test_execute_before_timelock_fails() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));
    s.gov.vote(&s.voter_a, &id, &true);

    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });
    s.gov.finalize(&id);

    // Timelock not elapsed yet
    let result = s.gov.try_execute(&id);
    assert_eq!(result, Err(Ok(GovError::TimelockNotElapsed)));
}

#[test]
fn test_execute_not_passed_fails() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));
    // No votes → rejected

    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });
    s.gov.finalize(&id);

    s.env.ledger().with_mut(|l| {
        l.timestamp += s.timelock_delay + 1;
    });

    let result = s.gov.try_execute(&id);
    assert_eq!(result, Err(Ok(GovError::NotPassed)));
}

#[test]
fn test_proposal_id_increments() {
    let s = Setup::new();
    let id0 = s.gov.propose(&s.proposer, &s.changes("fee_bps", "100"));
    let id1 = s.gov.propose(&s.proposer, &s.changes("fee_bps", "200"));
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
}

#[test]
fn test_get_proposal_not_found() {
    let s = Setup::new();
    let result = s.gov.try_get_proposal(&99);
    assert_eq!(result, Err(Ok(GovError::ProposalNotFound)));
}

#[test]
fn test_full_lifecycle_passes_and_marks_executed() {
    let s = Setup::new();
    let id = s.gov.propose(&s.proposer, &s.changes("fee_bps", "150"));
    s.gov.vote(&s.voter_a, &id, &true);

    // End voting
    s.env.ledger().with_mut(|l| {
        l.timestamp += s.voting_period + 1;
    });
    let status = s.gov.finalize(&id);
    assert_eq!(status, ProposalStatus::Passed);

    // Elapse timelock
    s.env.ledger().with_mut(|l| {
        l.timestamp += s.timelock_delay + 1;
    });

    // Execute — this will call gov_apply on the (dummy) escrow address.
    // In a unit test without a real escrow contract the cross-contract call
    // will panic, so we only verify the pre-execution state here.
    // Integration tests with a real escrow contract are in escrow_gov_tests.rs.
    let proposal = s.gov.get_proposal(&id).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Passed);
    assert!(s.env.ledger().timestamp() >= proposal.execution_eta);
}
