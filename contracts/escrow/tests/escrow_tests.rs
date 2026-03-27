#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, EscrowError};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
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
    contract: EscrowContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        Self::with_fee(0)
    }

    fn with_fee(fee_bps: u32) -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = Address::generate(&env);
        let freelancer = Address::generate(&env);
        let admin = Address::generate(&env);
        let fee_collector = Address::generate(&env);

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();

        token_admin.mint(&payer, &1000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);

        contract.init(&admin, &fee_bps, &fee_collector);

        Setup { env, payer, freelancer, token, token_addr, contract }
    }
}

// ── Happy path ────────────────────────────────────────────────────────────────

#[test]
fn test_full_happy_path() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Deliver MVP");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &None);

    assert_eq!(s.token.balance(&s.payer), 500);
    assert_eq!(s.token.balance(&s.contract.address), 500);

    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.token.balance(&s.freelancer), 500);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_cancel_refunds_payer() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Design mockups");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &300, &milestone, &None);
    assert_eq!(s.token.balance(&s.payer), 700);

    s.contract.cancel();

    assert_eq!(s.token.balance(&s.payer), 1000);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

// ── Typed error checks ────────────────────────────────────────────────────────

#[test]
fn test_cancel_after_submit_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Write tests");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &200, &milestone, &None);
    s.contract.submit_work();

    let err = s.contract.try_cancel().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::NotActive);
}

#[test]
fn test_approve_before_submit_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Deploy contract");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);

    let err = s.contract.try_approve().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::WorkNotSubmitted);
}

#[test]
fn test_double_create_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Phase 1");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);

    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::AlreadyExists);
}

#[test]
fn test_invalid_amount_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Phase 1");

    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &0, &milestone, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::InvalidAmount);
}

// ── Deadline / expire ─────────────────────────────────────────────────────────

#[test]
fn test_expire_before_deadline_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Expire test");

    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &Some(2000u64));

    let err = s.contract.try_expire().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::DeadlineNotPassed);
}

#[test]
fn test_expire_after_deadline_succeeds() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Expire test");

    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &Some(2000u64));

    s.env.ledger().with_mut(|l| l.timestamp = 3000);
    s.contract.expire();

    assert_eq!(s.token.balance(&s.payer), 1000);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_expire_without_deadline_fails() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "No deadline");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &None);

    let err = s.contract.try_expire().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::NotExpired);
}

// ── get_status ────────────────────────────────────────────────────────────────

#[test]
fn test_get_status_lifecycle() {
    use escrow::EscrowStatus;

    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Status test");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);
    assert_eq!(s.contract.get_status(), EscrowStatus::Active);

    s.contract.submit_work();
    assert_eq!(s.contract.get_status(), EscrowStatus::WorkSubmitted);

    s.contract.approve();
    assert_eq!(s.contract.get_status(), EscrowStatus::Completed);
}

#[test]
fn test_get_status_cancelled() {
    use escrow::EscrowStatus;

    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Cancel status");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);
    s.contract.cancel();
    assert_eq!(s.contract.get_status(), EscrowStatus::Cancelled);
}

#[test]
fn test_get_status_expired() {
    use escrow::EscrowStatus;

    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Expired status");

    s.env.ledger().with_mut(|l| l.timestamp = 100);
    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &Some(500u64));

    s.env.ledger().with_mut(|l| l.timestamp = 1000);
    s.contract.expire();
    assert_eq!(s.contract.get_status(), EscrowStatus::Expired);
}

// ── Issue #41: transfer_freelancer ────────────────────────────────────────────

#[test]
fn test_transfer_freelancer_and_submit_work() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Subcontract work");
    let new_freelancer = Address::generate(&s.env);

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &400, &milestone, &None);

    // Original freelancer transfers role
    s.contract.transfer_freelancer(&new_freelancer);

    // New freelancer submits work and gets paid
    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.token.balance(&new_freelancer), 400);
    assert_eq!(s.token.balance(&s.freelancer), 0);
}

#[test]
fn test_transfer_freelancer_updates_storage() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Transfer test");
    let new_freelancer = Address::generate(&s.env);

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);
    s.contract.transfer_freelancer(&new_freelancer);

    assert_eq!(s.contract.get_escrow().freelancer, new_freelancer);
}

// ── Issue #42: pause / unpause ────────────────────────────────────────────────

#[test]
fn test_pause_blocks_create() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Paused");

    s.contract.pause();

    let err = s.contract
        .try_create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, EscrowError::Paused);
}

#[test]
fn test_pause_blocks_submit_work() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Paused submit");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);
    s.contract.pause();

    let err = s.contract.try_submit_work().unwrap_err().unwrap();
    assert_eq!(err, EscrowError::Paused);
}

#[test]
fn test_unpause_restores_operations() {
    let s = Setup::new();
    let milestone = String::from_str(&s.env, "Unpause test");

    s.contract.pause();
    s.contract.unpause();

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &100, &milestone, &None);
    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.token.balance(&s.freelancer), 100);
}

// ── Issue #43: fee mechanism ──────────────────────────────────────────────────

#[test]
fn test_fee_deducted_on_approve() {
    // 100 bps = 1%; 500 * 100 / 10000 = 5 fee, freelancer gets 495
    let s = Setup::with_fee(100);
    let milestone = String::from_str(&s.env, "Fee test");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &None);
    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.token.balance(&s.freelancer), 495);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}

#[test]
fn test_zero_fee_full_payment() {
    let s = Setup::new(); // 0 bps
    let milestone = String::from_str(&s.env, "Zero fee");

    s.contract.create(&s.payer, &s.freelancer, &s.token_addr, &500, &milestone, &None);
    s.contract.submit_work();
    s.contract.approve();

    assert_eq!(s.token.balance(&s.freelancer), 500);
    assert_eq!(s.token.balance(&s.contract.address), 0);
}
