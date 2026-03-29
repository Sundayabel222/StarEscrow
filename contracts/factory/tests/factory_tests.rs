#![cfg(test)]

use factory::{EscrowFactory, EscrowFactoryClient, EscrowStatus, FactoryError};
use soroban_sdk::{
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &token_addr.address()),
        StellarAssetClient::new(env, &token_addr.address()),
    )
}

struct Setup<'a> {
    env: Env,
    payer: Address,
    freelancer: Address,
    token: TokenClient<'a>,
    token_addr: Address,
    factory: EscrowFactoryClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = Address::from_string(&String::from_str(&env, "payer"));
        let freelancer = Address::from_string(&String::from_str(&env, "freelancer"));
        let admin = Address::from_string(&String::from_str(&env, "admin"));

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();
        token_admin.mint(&payer, &10_000);

        let factory_addr = env.register_contract(None, EscrowFactory);
        let factory = EscrowFactoryClient::new(&env, &factory_addr);

        Setup { env, payer, freelancer, token, token_addr, factory }
    }

    fn create(&self, amount: i128, milestone: &str) -> u64 {
        let m = String::from_str(&self.env, milestone);
        self.factory.create_escrow(&self.payer, &self.freelancer, &self.token_addr, &amount, &m)
    }
}

// ── Factory creation ──────────────────────────────────────────────────────────

#[test]
fn test_factory_creates_escrow_with_unique_id() {
    let s = Setup::new();
    let id1 = s.create(100, "First");
    let id2 = s.create(200, "Second");
    assert_ne!(id1, id2);
    assert_eq!(id1, 0);
    assert_eq!(id2, 1);
}

#[test]
fn test_factory_locks_funds_on_create() {
    let s = Setup::new();
    s.create(500, "Lock test");
    assert_eq!(s.token.balance(&s.factory.address), 500);
    assert_eq!(s.token.balance(&s.payer), 9500);
}

#[test]
fn test_factory_invalid_amount_fails() {
    let s = Setup::new();
    let m = String::from_str(&s.env, "Bad");
    let err = s.factory
        .try_create_escrow(&s.payer, &s.freelancer, &s.token_addr, &0, &m)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, FactoryError::InvalidAmount);
}

// ── Happy path ────────────────────────────────────────────────────────────────

#[test]
fn test_factory_happy_path() {
    let s = Setup::new();
    let id = s.create(300, "Deliver feature");

    s.factory.submit_work(&id);
    s.factory.approve(&id);

    assert_eq!(s.token.balance(&s.freelancer), 300);
    assert_eq!(s.token.balance(&s.factory.address), 0);

    let record = s.factory.get_escrow(&id);
    assert_eq!(record.status, EscrowStatus::Completed);
}

#[test]
fn test_factory_cancel_refunds_payer() {
    let s = Setup::new();
    let id = s.create(400, "Cancel me");

    s.factory.cancel(&id);

    assert_eq!(s.token.balance(&s.payer), 10_000);
    assert_eq!(s.token.balance(&s.factory.address), 0);

    let record = s.factory.get_escrow(&id);
    assert_eq!(record.status, EscrowStatus::Cancelled);
}

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn test_factory_approve_before_submit_fails() {
    let s = Setup::new();
    let id = s.create(100, "Approve early");
    let err = s.factory.try_approve(&id).unwrap_err().unwrap();
    assert_eq!(err, FactoryError::WorkNotSubmitted);
}

#[test]
fn test_factory_cancel_after_submit_fails() {
    let s = Setup::new();
    let id = s.create(100, "Cancel after submit");
    s.factory.submit_work(&id);
    let err = s.factory.try_cancel(&id).unwrap_err().unwrap();
    assert_eq!(err, FactoryError::NotActive);
}

#[test]
fn test_factory_get_nonexistent_fails() {
    let s = Setup::new();
    let err = s.factory.try_get_escrow(&999u64).unwrap_err().unwrap();
    assert_eq!(err, FactoryError::NotFound);
}

// ── Listing ───────────────────────────────────────────────────────────────────

#[test]
fn test_factory_list_by_payer() {
    let s = Setup::new();
    let id1 = s.create(100, "A");
    let id2 = s.create(200, "B");
    let id3 = s.create(300, "C");

    let ids = s.factory.list_by_payer(&s.payer);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), id1);
    assert_eq!(ids.get(1).unwrap(), id2);
    assert_eq!(ids.get(2).unwrap(), id3);
}

#[test]
fn test_factory_list_by_freelancer() {
    let s = Setup::new();
    let id1 = s.create(100, "A");
    let id2 = s.create(200, "B");

    let ids = s.factory.list_by_freelancer(&s.freelancer);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids.get(0).unwrap(), id1);
    assert_eq!(ids.get(1).unwrap(), id2);
}

#[test]
fn test_factory_list_by_payer_empty_for_unknown() {
    let s = Setup::new();
    let unknown = Address::from_string(&String::from_str(&s.env, "unknown"));
    let ids = s.factory.list_by_payer(&unknown);
    assert_eq!(ids.len(), 0);
}

#[test]
fn test_factory_multiple_payers_isolated() {
    let s = Setup::new();
    let payer2 = Address::from_string(&String::from_str(&s.env, "payer2"));
    let (_, _token_admin) = create_token(&s.env, &Address::from_string(&String::from_str(&s.env, "token_admin")));
    // Mint for payer2 using the same token
    // (In practice we'd need the token admin — skip balance check, just verify index isolation)
    let m = String::from_str(&s.env, "P1 escrow");
    s.factory.create_escrow(&s.payer, &s.freelancer, &s.token_addr, &100, &m);

    // payer2 has no escrows
    let ids = s.factory.list_by_payer(&payer2);
    assert_eq!(ids.len(), 0);

    // payer has 1
    let ids = s.factory.list_by_payer(&s.payer);
    assert_eq!(ids.len(), 1);
}
