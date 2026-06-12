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

    struct AccountExpect {
        client: u16,
        available: Amount,
        held: Amount,
        locked: bool,
    }

    struct LedgerCase {
        name: &'static str,
        transactions: Vec<Transaction>,
        expected: Vec<AccountExpect>,
    }

    struct NoOpCase {
        name: &'static str,
        setup: Vec<Transaction>,
        extra: Vec<Transaction>,
        clients: Vec<u16>,
    }

    fn tx(kind: TransactionKind, client: u16, tx: u32, amount: Option<Amount>) -> Transaction {
        Transaction { kind, client, tx, amount }
    }

    fn assert_invariant(account: &Account, context: &str) {
        assert_eq!(
            account.available + account.held,
            account.total(),
            "{context}: invariant violated for client {}",
            account.client
        );
        assert!(account.held >= 0, "{context}: negative held for client {}", account.client);
    }

    fn apply_all_checked(transactions: &[Transaction], context: &str) -> Ledger {
        let mut ledger = Ledger::new();
        for transaction in transactions {
            ledger.apply(transaction);
            for account in ledger.accounts() {
                assert_invariant(account, context);
            }
        }
        ledger
    }

    fn assert_accounts(ledger: &Ledger, expected: &[AccountExpect], case_name: &str) {
        for expect in expected {
            let account = ledger
                .accounts()
                .find(|account| account.client == expect.client)
                .unwrap_or_else(|| panic!("{case_name}: missing client {}", expect.client));
            assert_eq!(account.available, expect.available, "{case_name}: available");
            assert_eq!(account.held, expect.held, "{case_name}: held");
            assert_eq!(account.locked, expect.locked, "{case_name}: locked");
            assert_eq!(account.total(), expect.available + expect.held, "{case_name}: total");
        }
    }

    fn run_ledger_cases(cases: &[LedgerCase]) {
        for case in cases {
            let ledger = apply_all_checked(&case.transactions, case.name);
            assert_accounts(&ledger, &case.expected, case.name);
        }
    }

    fn run_no_op_cases(cases: &[NoOpCase]) {
        for case in cases {
            let before = apply_all_checked(&case.setup, case.name);
            let mut combined = case.setup.clone();
            combined.extend_from_slice(&case.extra);
            let after = apply_all_checked(&combined, case.name);
            for &client in &case.clients {
                let before_account = before
                    .accounts()
                    .find(|account| account.client == client)
                    .expect("before account exists");
                let after_account = after
                    .accounts()
                    .find(|account| account.client == client)
                    .expect("after account exists");
                assert_eq!(
                    (before_account.available, before_account.held, before_account.locked),
                    (after_account.available, after_account.held, after_account.locked),
                    "{}",
                    case.name
                );
            }
        }
    }

    #[test]
    fn deposits_and_withdrawals_cases() {
        run_ledger_cases(&[
            LedgerCase {
                name: "deposit increases available and total",
                transactions: vec![tx(Deposit, 1, 1, Some(10_0000))],
                expected: vec![AccountExpect {
                    client: 1,
                    available: 10_0000,
                    held: 0,
                    locked: false,
                }],
            },
            LedgerCase {
                name: "withdrawal with sufficient funds decreases available and total",
                transactions: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Withdrawal, 1, 2, Some(4_0000)),
                ],
                expected: vec![AccountExpect {
                    client: 1,
                    available: 6_0000,
                    held: 0,
                    locked: false,
                }],
            },
            LedgerCase {
                name: "multiple clients have independent balances",
                transactions: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Deposit, 2, 2, Some(20_0000)),
                    tx(Withdrawal, 1, 3, Some(3_0000)),
                ],
                expected: vec![
                    AccountExpect { client: 1, available: 7_0000, held: 0, locked: false },
                    AccountExpect { client: 2, available: 20_0000, held: 0, locked: false },
                ],
            },
            LedgerCase {
                name: "client account is auto-created on first transaction",
                transactions: vec![tx(Withdrawal, 42, 1, Some(1_0000))],
                expected: vec![AccountExpect { client: 42, available: 0, held: 0, locked: false }],
            },
        ]);

        run_no_op_cases(&[NoOpCase {
            name: "withdrawal with insufficient funds is no-op",
            setup: vec![tx(Deposit, 1, 1, Some(5_0000))],
            extra: vec![tx(Withdrawal, 1, 2, Some(6_0000))],
            clients: vec![1],
        }]);
    }

    #[test]
    fn dispute_lifecycle_cases() {
        run_ledger_cases(&[
            LedgerCase {
                name: "dispute moves funds from available to held",
                transactions: vec![tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)],
                expected: vec![AccountExpect {
                    client: 1,
                    available: 0,
                    held: 10_0000,
                    locked: false,
                }],
            },
            LedgerCase {
                name: "deposit dispute resolve restores original available",
                transactions: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Resolve, 1, 1, None),
                ],
                expected: vec![AccountExpect {
                    client: 1,
                    available: 10_0000,
                    held: 0,
                    locked: false,
                }],
            },
            LedgerCase {
                name: "deposit dispute chargeback removes funds and locks",
                transactions: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Chargeback, 1, 1, None),
                ],
                expected: vec![AccountExpect { client: 1, available: 0, held: 0, locked: true }],
            },
            LedgerCase {
                name: "dispute after withdrawing proceeds allows negative available",
                transactions: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Withdrawal, 1, 2, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                ],
                expected: vec![AccountExpect {
                    client: 1,
                    available: -10_0000,
                    held: 10_0000,
                    locked: false,
                }],
            },
        ]);
    }

    #[test]
    fn invalid_edge_cases() {
        run_no_op_cases(&[
            NoOpCase {
                name: "dispute on unknown tx is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000))],
                extra: vec![tx(Dispute, 1, 99, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "dispute on withdrawal tx is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000)), tx(Withdrawal, 1, 2, Some(3_0000))],
                extra: vec![tx(Dispute, 1, 2, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "resolve on non-disputed tx is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000))],
                extra: vec![tx(Resolve, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "chargeback on non-disputed tx is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000))],
                extra: vec![tx(Chargeback, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "second dispute on same tx is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)],
                extra: vec![tx(Dispute, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "dispute with mismatched client is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000))],
                extra: vec![tx(Dispute, 2, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "resolve with mismatched client is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)],
                extra: vec![tx(Resolve, 2, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "chargeback with mismatched client is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(10_0000)), tx(Dispute, 1, 1, None)],
                extra: vec![tx(Chargeback, 2, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "resolve on already resolved tx is ignored",
                setup: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Resolve, 1, 1, None),
                ],
                extra: vec![tx(Resolve, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "chargeback on already resolved tx is ignored",
                setup: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Resolve, 1, 1, None),
                ],
                extra: vec![tx(Chargeback, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "second chargeback on same tx is ignored",
                setup: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Chargeback, 1, 1, None),
                ],
                extra: vec![tx(Chargeback, 1, 1, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "withdrawal without amount is ignored",
                setup: vec![tx(Deposit, 1, 1, Some(5_0000))],
                extra: vec![tx(Withdrawal, 1, 2, None)],
                clients: vec![1],
            },
            NoOpCase {
                name: "locked account ignores all subsequent transactions",
                setup: vec![
                    tx(Deposit, 1, 1, Some(10_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Chargeback, 1, 1, None),
                ],
                extra: vec![
                    tx(Deposit, 1, 2, Some(5_0000)),
                    tx(Withdrawal, 1, 3, Some(1_0000)),
                    tx(Dispute, 1, 1, None),
                    tx(Resolve, 1, 1, None),
                    tx(Chargeback, 1, 1, None),
                ],
                clients: vec![1],
            },
        ]);

        let ledger =
            apply_all_checked(&[tx(Deposit, 1, 1, None)], "deposit without amount is ignored");
        assert!(ledger.accounts().next().is_none(), "deposit without amount is ignored");
    }

    #[test]
    fn invariant_holds_after_every_apply() {
        let transactions = vec![
            tx(Deposit, 1, 1, Some(10_0000)),
            tx(Deposit, 2, 2, Some(5_0000)),
            tx(Withdrawal, 1, 3, Some(3_0000)),
            tx(Dispute, 1, 1, None),
            tx(Resolve, 1, 1, None),
            tx(Deposit, 3, 4, Some(8_0000)),
            tx(Dispute, 3, 4, None),
            tx(Chargeback, 3, 4, None),
            tx(Deposit, 3, 5, Some(1_0000)),
        ];

        let ledger = apply_all_checked(&transactions, "invariant composite stream");

        assert_accounts(
            &ledger,
            &[
                AccountExpect { client: 1, available: 7_0000, held: 0, locked: false },
                AccountExpect { client: 2, available: 5_0000, held: 0, locked: false },
                AccountExpect { client: 3, available: 0, held: 0, locked: true },
            ],
            "invariant composite stream",
        );
    }
}
