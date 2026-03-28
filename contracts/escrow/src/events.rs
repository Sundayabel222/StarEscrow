use soroban_sdk::{Address, Env, String, Symbol};

pub fn escrow_created(
    env: &Env,
    payer: &Address,
    freelancer: &Address,
    amount: i128,
    milestone: &String,
) {
    env.events().publish(
        (Symbol::new(env, "escrow_created"),),
        (payer.clone(), freelancer.clone(), amount, milestone.clone()),
    );
}

pub fn work_submitted(env: &Env, freelancer: &Address) {
    env.events()
        .publish((Symbol::new(env, "work_submitted"),), (freelancer.clone(),));
}

pub fn payment_released(env: &Env, freelancer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "payment_released"),),
        (freelancer.clone(), amount),
    );
}

pub fn escrow_cancelled(env: &Env, payer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_cancelled"),),
        (payer.clone(), amount),
    );
}

pub fn escrow_expired(env: &Env, payer: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "escrow_expired"),),
        (payer.clone(), amount),
    );
}

pub fn freelancer_transferred(env: &Env, old: &Address, new: &Address) {
    env.events().publish(
        (Symbol::new(env, "freelancer_transferred"),),
        (old.clone(), new.clone()),
    );
}

pub fn contract_paused(env: &Env, admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "contract_paused"),),
        (admin.clone(),),
    );
}

pub fn contract_unpaused(env: &Env, admin: &Address) {
    env.events().publish(
        (Symbol::new(env, "contract_unpaused"),),
        (admin.clone(),),
    );
}

pub fn yield_deposited(env: &Env, protocol: &Address, amount: i128) {
    env.events().publish(
        (Symbol::new(env, "yield_deposited"),),
        (protocol.clone(), amount),
    );
}

pub fn recurring_released(env: &Env, freelancer: &Address, amount: i128, release_num: u32) {
    env.events().publish(
        (Symbol::new(env, "recurring_released"),),
        (freelancer.clone(), amount, release_num),
    );
}
