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
    Unauthorized = 7,
    Paused = 8,
    TokenNotAllowed = 9,
    IntervalNotElapsed = 10,
    RecurrenceComplete = 11,
    NotRecurring = 12,
    MilestoneInvalidIndex = 13,
    MilestoneNotPending = 14,
    MilestoneNotSubmitted = 15,
    MilestoneAlreadyApproved = 16,
}
