#![cfg(test)]

use reputation::{ReputationContract, ReputationContractClient, ReputationError, ReputationStats};
use soroban_sdk::{Address, Env, String};

struct Setup<'a> {
    env: Env,
    #[allow(dead_code)]
    admin: Address,
    contract: ReputationContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::from_string(&String::from_str(&env, "admin"));
        let contract_addr = env.register_contract(None, ReputationContract);
        let contract = ReputationContractClient::new(&env, &contract_addr);
        contract.init(&admin);

        Setup { env, admin, contract }
    }

    fn register_caller(&self, caller: &Address) {
        self.contract.register_caller(caller);
    }
}

// ── Initialization ────────────────────────────────────────────────────────────

#[test]
fn test_double_init_fails() {
    let s = Setup::new();
    let admin2 = Address::from_string(&String::from_str(&s.env, "admin2"));
    let err = s.contract.try_init(&admin2).unwrap_err().unwrap();
    assert_eq!(err, ReputationError::AlreadyInitialized);
}

// ── Unauthorized caller ───────────────────────────────────────────────────────

#[test]
fn test_unregistered_caller_rejected() {
    let s = Setup::new();
    let unregistered = Address::from_string(&String::from_str(&s.env, "unregistered"));
    let freelancer = Address::from_string(&String::from_str(&s.env, "freelancer"));

    let err = s.contract
        .try_record_completion(&unregistered, &freelancer)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, ReputationError::Unauthorized);
}

// ── Reputation accumulation ───────────────────────────────────────────────────

#[test]
fn test_completion_increments_score() {
    let s = Setup::new();
    let escrow = Address::from_string(&String::from_str(&s.env, "escrow"));
    let freelancer = Address::from_string(&String::from_str(&s.env, "freelancer"));
    s.register_caller(&escrow);

    s.contract.record_completion(&escrow, &freelancer);
    s.contract.record_completion(&escrow, &freelancer);

    let stats = s.contract.get_stats(&freelancer);
    assert_eq!(stats.completed, 2);
    assert_eq!(stats.cancelled, 0);
    assert_eq!(s.contract.get_reputation(&freelancer), 20); // 2 * 10
}

#[test]
fn test_cancellation_decrements_score() {
    let s = Setup::new();
    let escrow = Address::from_string(&String::from_str(&s.env, "escrow"));
    let freelancer = Address::from_string(&String::from_str(&s.env, "freelancer"));
    s.register_caller(&escrow);

    s.contract.record_completion(&escrow, &freelancer);
    s.contract.record_cancellation(&escrow, &freelancer);

    let stats = s.contract.get_stats(&freelancer);
    assert_eq!(stats.completed, 1);
    assert_eq!(stats.cancelled, 1);
    assert_eq!(s.contract.get_reputation(&freelancer), 5); // 10 - 5
}

#[test]
fn test_score_floors_at_zero() {
    let s = Setup::new();
    let escrow = Address::from_string(&String::from_str(&s.env, "escrow"));
    let freelancer = Address::from_string(&String::from_str(&s.env, "freelancer"));
    s.register_caller(&escrow);

    // 3 cancellations, 0 completions → 0 - 15 = saturates to 0
    s.contract.record_cancellation(&escrow, &freelancer);
    s.contract.record_cancellation(&escrow, &freelancer);
    s.contract.record_cancellation(&escrow, &freelancer);

    assert_eq!(s.contract.get_reputation(&freelancer), 0);
}

#[test]
fn test_multiple_escrows_accumulate() {
    let s = Setup::new();
    let escrow1 = Address::from_string(&String::from_str(&s.env, "escrow1"));
    let escrow2 = Address::from_string(&String::from_str(&s.env, "escrow2"));
    let freelancer = Address::from_string(&String::from_str(&s.env, "freelancer"));
    s.register_caller(&escrow1);
    s.register_caller(&escrow2);

    s.contract.record_completion(&escrow1, &freelancer);
    s.contract.record_completion(&escrow2, &freelancer);
    s.contract.record_completion(&escrow2, &freelancer);

    assert_eq!(s.contract.get_reputation(&freelancer), 30); // 3 * 10
}

#[test]
fn test_unknown_address_has_zero_reputation() {
    let s = Setup::new();
    let unknown = Address::from_string(&String::from_str(&s.env, "unknown"));
    assert_eq!(s.contract.get_reputation(&unknown), 0);
    let stats = s.contract.get_stats(&unknown);
    assert_eq!(stats, ReputationStats { completed: 0, cancelled: 0 });
}

#[test]
fn test_different_addresses_isolated() {
    let s = Setup::new();
    let escrow = Address::from_string(&String::from_str(&s.env, "escrow"));
    let freelancer1 = Address::from_string(&String::from_str(&s.env, "freelancer1"));
    let freelancer2 = Address::from_string(&String::from_str(&s.env, "freelancer2"));
    s.register_caller(&escrow);

    s.contract.record_completion(&escrow, &freelancer1);
    s.contract.record_completion(&escrow, &freelancer1);
    s.contract.record_cancellation(&escrow, &freelancer2);

    assert_eq!(s.contract.get_reputation(&freelancer1), 20);
    assert_eq!(s.contract.get_reputation(&freelancer2), 0);
}
