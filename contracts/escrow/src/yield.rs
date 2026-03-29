use soroban_sdk::{contractclient, Env};

#[allow(dead_code)]
#[contractclient(name = "YieldProtocolClient")]
pub trait YieldProtocol {
    fn deposit(env: Env, amount: i128);
    fn withdraw(env: Env, requested: i128) -> (i128, i128);
}
