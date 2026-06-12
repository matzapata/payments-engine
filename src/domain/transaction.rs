/// Fixed-point money scaled ×10⁴ (four decimal places).
pub type Amount = i64;

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
