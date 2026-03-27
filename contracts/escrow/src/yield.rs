use soroban_sdk::{contractclient, contracttype, Env};

#[contractclient(name = "YieldProtocolClient")]
pub struct YieldProtocolClient<'a, E: Env> {
    env: E,
}

#[contractclient]
impl<E: Env> YieldProtocolClient<'a, E> {
    /// Deposit amount from caller to this contract.
    pub fn deposit(env: &Env, amount: &i128);

    /// Withdraw requested principal, returns (principal_returned, yield_accrued).
    /// Transfers principal + yield back to caller.
    pub fn withdraw(env: &Env, requested: &i128) -> (i128, i128);
}
