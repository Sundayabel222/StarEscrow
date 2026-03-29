use soroban_sdk::{contractclient, Address, Env};

#[allow(dead_code)]
#[contractclient(name = "ReputationContractClient")]
pub trait ReputationContract {
    fn record_completion(env: Env, caller: Address, address: Address);
    fn record_cancellation(env: Env, caller: Address, address: Address);
}
