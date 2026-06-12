use std::collections::HashMap;

use super::account::Account;
use super::stored_transaction::StoredTransaction;
use super::transaction::{DisputeState, Transaction, TransactionKind, checked_add, checked_sub};

#[derive(Debug, Default)]
pub struct Ledger {
    accounts: HashMap<u16, Account>,
    transactions: HashMap<u32, StoredTransaction>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_transactions(transactions: &[Transaction]) -> Self {
        let mut ledger = Self::new();
        for transaction in transactions {
            ledger.apply(transaction);
        }
        ledger
    }

    pub fn accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts.values()
    }

    pub fn apply(&mut self, transaction: &Transaction) {
        if self.is_client_locked(transaction.client) {
            return;
        }

        match transaction.kind {
            TransactionKind::Deposit => self.apply_deposit(transaction),
            TransactionKind::Withdrawal => self.apply_withdrawal(transaction),
            TransactionKind::Dispute => self.apply_dispute(transaction),
            TransactionKind::Resolve => self.apply_resolve(transaction),
            TransactionKind::Chargeback => self.apply_chargeback(transaction),
        }
    }

    fn is_client_locked(&self, client: u16) -> bool {
        self.accounts.get(&client).is_some_and(|account| account.locked)
    }

    fn account_mut(&mut self, client: u16) -> &mut Account {
        self.accounts.entry(client).or_insert_with(|| Account::new(client))
    }

    fn apply_deposit(&mut self, transaction: &Transaction) {
        let Some(amount) = transaction.amount else {
            return;
        };

        {
            let account = self.account_mut(transaction.client);
            account.available = match checked_add(account.available, amount) {
                Some(available) => available,
                None => return,
            };
        }

        self.transactions.insert(
            transaction.tx,
            StoredTransaction {
                client: transaction.client,
                amount,
                dispute_state: DisputeState::None,
            },
        );

        self.accounts.get_mut(&transaction.client).expect("account exists").assert_invariant();
    }

    fn apply_withdrawal(&mut self, transaction: &Transaction) {
        let Some(amount) = transaction.amount else {
            return;
        };

        let account = self.account_mut(transaction.client);
        if account.available < amount {
            return;
        }

        account.available = match checked_sub(account.available, amount) {
            Some(available) => available,
            None => return,
        };

        account.assert_invariant();
    }

    fn apply_dispute(&mut self, transaction: &Transaction) {
        let stored = match self.transactions.get(&transaction.tx) {
            Some(stored) => *stored,
            None => return,
        };

        if stored.client != transaction.client {
            return;
        }
        if stored.dispute_state != DisputeState::None {
            return;
        }

        let amount = stored.amount;
        let client = transaction.client;

        {
            let account = self.account_mut(client);
            let (new_available, new_held) =
                match (checked_sub(account.available, amount), checked_add(account.held, amount)) {
                    (Some(available), Some(held)) => (available, held),
                    _ => return,
                };
            account.available = new_available;
            account.held = new_held;
        }

        self.transactions.get_mut(&transaction.tx).expect("tx exists").dispute_state =
            DisputeState::Disputed;

        self.accounts.get_mut(&client).expect("account exists").assert_invariant();
    }

    fn apply_resolve(&mut self, transaction: &Transaction) {
        let stored = match self.transactions.get(&transaction.tx) {
            Some(stored) => *stored,
            None => return,
        };

        if stored.client != transaction.client {
            return;
        }
        if stored.dispute_state != DisputeState::Disputed {
            return;
        }

        let amount = stored.amount;
        let client = stored.client;

        {
            let account = self.account_mut(client);
            let (new_held, new_available) =
                match (checked_sub(account.held, amount), checked_add(account.available, amount)) {
                    (Some(held), Some(available)) => (held, available),
                    _ => return,
                };
            account.held = new_held;
            account.available = new_available;
        }

        self.transactions.get_mut(&transaction.tx).expect("tx exists").dispute_state =
            DisputeState::Resolved;

        self.accounts.get_mut(&client).expect("account exists").assert_invariant();
    }

    fn apply_chargeback(&mut self, transaction: &Transaction) {
        let stored = match self.transactions.get(&transaction.tx) {
            Some(stored) => *stored,
            None => return,
        };

        if stored.client != transaction.client {
            return;
        }
        if stored.dispute_state != DisputeState::Disputed {
            return;
        }

        let amount = stored.amount;
        let client = stored.client;

        {
            let account = self.account_mut(client);
            account.held = match checked_sub(account.held, amount) {
                Some(held) => held,
                None => return,
            };
            account.locked = true;
        }

        self.transactions.get_mut(&transaction.tx).expect("tx exists").dispute_state =
            DisputeState::ChargedBack;

        self.accounts.get_mut(&client).expect("account exists").assert_invariant();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Amount;
    use crate::domain::transaction::TransactionKind::*;

    fn tx(kind: TransactionKind, client: u16, tx: u32, amount: Option<Amount>) -> Transaction {
        Transaction { kind, client, tx, amount }
    }

    fn account(ledger: &Ledger, client: u16) -> Account {
        *ledger.accounts().find(|account| account.client == client).expect("account exists")
    }

    fn snapshot(ledger: &Ledger, client: u16) -> (Amount, Amount, bool) {
        let account = account(ledger, client);
        (account.available, account.held, account.locked)
    }

    #[test]
    fn deposit_increases_available_and_total() {
        let ledger = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000))]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 10_0000);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 10_0000);
        assert!(!account.locked);
    }

    #[test]
    fn withdrawal_with_sufficient_funds_decreases_available_and_total() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Withdrawal, 1, 2, Some(4_0000)),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 6_0000);
        assert_eq!(account.total(), 6_0000);
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_no_op() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(5_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(5_0000)),
            tx(Withdrawal, 1, 2, Some(6_0000)),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn client_account_is_auto_created() {
        let ledger = Ledger::from_transactions(&[tx(Withdrawal, 42, 1, Some(1_0000))]);
        let account = account(&ledger, 42);
        assert_eq!(account.available, 0);
        assert_eq!(account.held, 0);
    }

    #[test]
    fn multiple_clients_have_independent_balances() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Deposit, 2, 2, Some(20_0000)),
            tx(Withdrawal, 1, 3, Some(3_0000)),
        ]);
        assert_eq!(account(&ledger, 1).available, 7_0000);
        assert_eq!(account(&ledger, 2).available, 20_0000);
    }

    #[test]
    fn dispute_moves_funds_from_available_to_held() {
        let ledger =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 0);
        assert_eq!(account.held, 10_0000);
        assert_eq!(account.total(), 10_0000);
    }

    #[test]
    fn dispute_on_unknown_tx_is_ignored() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 99, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn dispute_on_withdrawal_tx_is_ignored() {
        let before = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Withdrawal, 1, 2, Some(3_0000)),
        ]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Withdrawal, 1, 2, Some(3_0000)),
            tx(Dispute, 1, 2, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn resolve_releases_held_funds_to_available() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 10_0000);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 10_0000);
    }

    #[test]
    fn chargeback_removes_held_funds_and_locks_account() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Chargeback, 1, 1, None),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 0);
        assert_eq!(account.held, 0);
        assert_eq!(account.total(), 0);
        assert!(account.locked);
    }

    #[test]
    fn deposit_dispute_resolve_restores_original_available() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 10_0000);
        assert_eq!(account.held, 0);
    }

    #[test]
    fn deposit_dispute_chargeback_removes_funds_and_locks() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Chargeback, 1, 1, None),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, 0);
        assert_eq!(account.total(), 0);
        assert!(account.locked);
    }

    #[test]
    fn second_dispute_on_same_tx_is_ignored() {
        let before =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Dispute, 1, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn resolve_on_non_disputed_tx_is_ignored() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Resolve, 1, 1, None)]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn chargeback_on_non_disputed_tx_is_ignored() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Chargeback, 1, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn dispute_with_mismatched_client_is_ignored() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 2, 1, None)]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn locked_account_ignores_all_subsequent_transactions() {
        let locked = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Chargeback, 1, 1, None),
        ]);
        let locked_snapshot = snapshot(&locked, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Chargeback, 1, 1, None),
            tx(Deposit, 1, 2, Some(5_0000)),
            tx(Withdrawal, 1, 3, Some(1_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
            tx(Chargeback, 1, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), locked_snapshot);
    }

    #[test]
    fn dispute_after_withdrawing_proceeds_allows_negative_available() {
        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Withdrawal, 1, 2, Some(10_0000)),
            tx(Dispute, 1, 1, None),
        ]);
        let account = account(&ledger, 1);
        assert_eq!(account.available, -10_0000);
        assert_eq!(account.held, 10_0000);
        assert_eq!(account.total(), 0);
    }

    #[test]
    fn deposit_without_amount_is_ignored() {
        let ledger = Ledger::from_transactions(&[tx(Deposit, 1, 1, None)]);
        assert!(ledger.accounts().next().is_none());
    }

    #[test]
    fn withdrawal_without_amount_is_ignored() {
        let before = Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(5_0000))]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(5_0000)),
            tx(Withdrawal, 1, 2, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn resolve_with_mismatched_client_is_ignored() {
        let before =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 2, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn chargeback_with_mismatched_client_is_ignored() {
        let before =
            Ledger::from_transactions(&[tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Chargeback, 2, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }

    #[test]
    fn resolve_on_already_resolved_tx_is_ignored() {
        let before = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
        ]);
        let before_snapshot = snapshot(&before, 1);

        let ledger = Ledger::from_transactions(&[
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
            tx(Resolve, 1, 1, None),
        ]);

        assert_eq!(snapshot(&ledger, 1), before_snapshot);
    }
}
