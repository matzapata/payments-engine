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
        Self { client, available: 0, held: 0, locked: false }
    }

    pub fn total(&self) -> Amount {
        self.available + self.held
    }

    pub(crate) fn assert_invariant(&self) {
        debug_assert!(self.held >= 0);
    }
}
