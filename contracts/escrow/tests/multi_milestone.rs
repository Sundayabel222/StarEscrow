#![cfg(test)]

use super::*;

#[test]
fn test_multi_milestone_happy_path() {
    let s = Setup::new();
    let m1 = storage::Milestone {
        description: String::from_str(&s.env, "Milestone 1"),
        amount: 300,
        status: storage::MilestoneStatus::Pending,
    };
    let m2 = storage::Milestone {
        description: String::from_str(&s.env, "Milestone 2"),
        amount: 200,
        status: storage::MilestoneStatus::Pending,
    };
    let milestones = vec![&s.env, m1, m2];
    s.contract.create(
        &s.payer,
        &s.freelancer,
        &s.token_addr,
        milestones,
        &None::<u64>,
        &None::<Address>,
        &storage::YieldRecipient::Payer,
        &0u64,
        &0u32,
    );

    // Submit milestone 0
    s.contract.submit_work(0);
    // Approve milestone 0
    s.contract.approve(0);
    assert_eq!(s.token.balance(&s.freelancer), 300);
    assert_eq!(s.token.balance(&s.contract.address), 200);

    // Submit milestone 1
    s.contract.submit_work(1);
    // Approve milestone 1
    s.contract.approve(1);
    assert_eq!(s.token.balance(&s.freelancer), 500);
    assert_eq!(s.contract.get_status(), EscrowStatus::Completed);
}
