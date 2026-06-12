/// Fixed-point money scaled ×10⁴ (four decimal places).
pub type Amount = i64;

pub fn checked_add(a: Amount, b: Amount) -> Option<Amount> {
    a.checked_add(b)
}

pub fn checked_sub(a: Amount, b: Amount) -> Option<Amount> {
    a.checked_sub(b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeState {
    None,
    Disputed,
    Resolved,
    ChargedBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Amount>,
}
