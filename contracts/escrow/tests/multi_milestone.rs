#![cfg(test)]

use escrow::{EscrowContract, EscrowContractClient, YieldRecipient, storage::{Milestone, MilestoneStatus}};
use escrow::storage;
use soroban_sdk::{token::{Client as TokenClient, StellarAssetClient}, Address, Env, String, Vec};

fn create_token<'a>(env: &Env, admin: &Address) -> (TokenClient<'a>, StellarAssetClient<'a>) {
    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    (
        TokenClient::new(env, &token_addr.address()),
        StellarAssetClient::new(env, &token_addr.address()),
    )
}

fn test_address(name: &str) -> Address {
    let env = Env::default();
    let bytes = name.as_bytes();
    let mut addr_bytes = [0u8; 32];
    for (i, &byte) in bytes.iter().enumerate().take(32) {
        addr_bytes[i] = byte;
    }
    let strkey = String::from_str(&env, name);
    Address::from_string(&strkey)
}

struct Setup<'a> {
    env: Env,
    payer: Address,
    freelancer: Address,
    arbitrator: Address,
    token: TokenClient<'a>,
    token_addr: Address,
    contract: EscrowContractClient<'a>,
}

impl<'a> Setup<'a> {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let payer = test_address("payer");
        let freelancer = test_address("freelancer");
        let arbitrator = test_address("arbitrator");
        let admin = test_address("admin");
        let fee_collector = test_address("fee_collector");

        let (token, token_admin) = create_token(&env, &admin);
        let token_addr = token.address.clone();
        token_admin.mint(&payer, &10_000);

        let contract_addr = env.register_contract(None, EscrowContract);
        let contract = EscrowContractClient::new(&env, &contract_addr);
        contract.init(&admin, &0u32, &fee_collector);

        Setup { env, payer, freelancer, arbitrator, token, token_addr, contract }
    }
}

#[test]
fn test_multi_milestone_happy_path() {
    let s = Setup::new();
    let m1 = Milestone {
        description: String::from_str(&s.env, "Milestone 1"),
        amount: 300,
        status: MilestoneStatus::Pending,
    };
    let m2 = Milestone {
        description: String::from_str(&s.env, "Milestone 2"),
        amount: 200,
        status: MilestoneStatus::Pending,
    };
    let mut milestones = Vec::new(&s.env);
    milestones.push_back(m1);
    milestones.push_back(m2);

    let config = storage::EscrowConfig {
        deadline: None,
        yield_protocol: None,
        yield_recipient: YieldRecipient::Payer,
        interval: 0u64,
        recurrence_count: 0u32,
    };
    s.contract.create_with_milestones(
        &s.payer,
        &s.freelancer,
        &s.arbitrator,
        &s.token_addr,
        &milestones,
        &config,
    );

    // Submit milestone 0
    s.contract.submit_work(&0u32);
    // Approve milestone 0
    s.contract.approve(&0u32);
    assert_eq!(s.token.balance(&s.freelancer), 300);
    assert_eq!(s.token.balance(&s.contract.address), 200);

    // Submit milestone 1
    s.contract.submit_work(&1u32);
    // Approve milestone 1
    s.contract.approve(&1u32);
    assert_eq!(s.token.balance(&s.freelancer), 500);
    assert_eq!(s.contract.get_status(), escrow::EscrowStatus::Completed);
}
