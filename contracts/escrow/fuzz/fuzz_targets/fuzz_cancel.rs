#![no_main]

use arbitrary::Arbitrary;
use escrow::{EscrowContract, EscrowContractClient, YieldRecipient};
use escrow::storage::{Milestone, MilestoneStatus};
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, Env, String, Vec,
};

/// Fuzz input for the `cancel` path.
#[derive(Arbitrary, Debug)]
struct CancelInput {
    /// Amount for the single milestone.
    amount: i128,
    /// Whether to submit work before cancelling (cancel should fail after submission).
    submit_first: bool,
}

fuzz_target!(|input: CancelInput| {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let payer = Address::generate(&env);
    let freelancer = Address::generate(&env);
    let fee_collector = Address::generate(&env);

    let token_addr = env.register_stellar_asset_contract_v2(admin.clone());
    let token_admin = StellarAssetClient::new(&env, &token_addr.address());
    token_admin.mint(&payer, &i128::MAX);

    let contract_addr = env.register_contract(None, EscrowContract);
    let contract = EscrowContractClient::new(&env, &contract_addr);
    contract.init(&admin, &0u32, &fee_collector);

    let s = String::from_str(&env, "milestone");
    let mut milestones: Vec<Milestone> = Vec::new(&env);
    milestones.push_back(Milestone {
        description: s,
        amount: input.amount,
        status: MilestoneStatus::Pending,
    });

    if contract
        .try_create(
            &payer,
            &freelancer,
            &token_addr.address(),
            &milestones,
            &None,
            &None,
            &YieldRecipient::Payer,
            &0u64,
            &0u32,
        )
        .is_err()
    {
        return;
    }

    if input.submit_first {
        // After submission, cancel must return NotActive — not panic.
        let _ = contract.try_submit_work(&0u32);
    }

    // cancel must not panic regardless of state.
    let _ = contract.try_cancel();
});
