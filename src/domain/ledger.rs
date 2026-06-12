use std::collections::HashMap;

use super::account::Account;
use super::transaction::Transaction;

#[derive(Debug, Default)]
pub struct Ledger {
    accounts: HashMap<u16, Account>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    pub fn apply(&mut self, _transaction: &Transaction) {
        // Payment and dispute rules will be implemented here.
    }
}
