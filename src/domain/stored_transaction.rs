use super::transaction::{Amount, DisputeState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoredTransaction {
    pub client: u16,
    pub amount: Amount,
    pub dispute_state: DisputeState,
}
