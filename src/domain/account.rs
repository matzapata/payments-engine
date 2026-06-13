use super::transaction::Amount;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Account {
    pub client: u16,
    pub available: Amount,
    pub held: Amount,
    pub locked: bool,
}

impl Account {
    pub fn new(client: u16) -> Self {
        Self { client, available: Amount::ZERO, held: Amount::ZERO, locked: false }
    }

    /// Returns `available + held`, or `None` if the sum would overflow `Amount`.
    ///
    /// The ledger's `apply_*` paths use `checked_add` / `checked_sub` so a `None` here
    /// represents a violated invariant (or test data outside the supported range), not
    /// a transient state — callers can safely panic or surface a hard error.
    pub fn total(&self) -> Option<Amount> {
        self.available.checked_add(self.held)
    }

    pub(crate) fn assert_invariant(&self) {
        debug_assert!(self.held >= Amount::ZERO);
        debug_assert!(self.total().is_some(), "available + held overflows Amount");
    }
}
