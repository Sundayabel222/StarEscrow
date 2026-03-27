use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EscrowError {
    AlreadyExists = 1,
    NotActive = 2,
    WorkNotSubmitted = 3,
    InvalidAmount = 4,
    DeadlineNotPassed = 5,
    NotExpired = 6,
}
