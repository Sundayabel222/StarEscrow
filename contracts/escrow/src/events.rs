use soroban_sdk::{Address, Env, String, Symbol};

/// Emitted when an escrow is created and funds are locked.
pub fn escrow_created(env: &Env, payer: &Address, freelancer: &Address, amount: i128, milestone: &String) {
    env.events().publish(
        (Symbol::new(env, "escrow_created"),),
        (payer.clone(), freelancer.clone(), amount, milestone.clone()),
    );
}

/// Emitted when the freelancer submits their work.
pub fn work_submitted(env: &Env, freelancer: &Address) {
    env.events().publish(
        (Symbol::new(env, "work_submitted"),),
        (freelancer.clone(),),
    );
}

/// Emitted when the payer approves and funds are released.
pub fn payment_released(env: &Env, freelancer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "payment_released"),),
        (freelancer.clone(), amount),
    );
}

/// Emitted when the payer cancels and funds are refunded.
pub fn escrow_cancelled(env: &Env, payer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_cancelled"),),
        (payer.clone(), amount),
    );
}
