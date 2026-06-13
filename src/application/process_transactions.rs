use crate::domain::ledger::Ledger;
use crate::infrastructure::accounts::AccountSink;
use crate::infrastructure::transactions::TransactionSource;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("csv error: {0}")]
    Csv(#[from] csv::Error),
}

pub fn run<S, A>(source: &mut S, sink: &mut A) -> Result<(), AppError>
where
    S: TransactionSource,
    S::Error: Into<AppError>,
    A: AccountSink,
    A::Error: Into<AppError>,
{
    let mut ledger = Ledger::new();
    while let Some(result) = source.next_transaction() {
        ledger.apply(&result.map_err(Into::into)?);
    }
    sink.write_accounts(ledger.accounts().copied()).map_err(Into::into)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::Account;
    use crate::domain::transaction::{Amount, Transaction, TransactionKind};

    struct VecSource {
        transactions: std::vec::IntoIter<Transaction>,
    }

    impl VecSource {
        fn new(transactions: Vec<Transaction>) -> Self {
            Self { transactions: transactions.into_iter() }
        }
    }

    impl TransactionSource for VecSource {
        type Error = AppError;

        fn next_transaction(&mut self) -> Option<Result<Transaction, Self::Error>> {
            self.transactions.next().map(Ok)
        }
    }

    struct VecSink {
        accounts: Vec<Account>,
    }

    impl VecSink {
        fn new() -> Self {
            Self { accounts: Vec::new() }
        }
    }

    impl AccountSink for VecSink {
        type Error = AppError;

        fn write_accounts(
            &mut self,
            accounts: impl IntoIterator<Item = Account>,
        ) -> Result<(), Self::Error> {
            self.accounts.extend(accounts);
            Ok(())
        }
    }

    fn tx(kind: TransactionKind, client: u16, tx: u32, amount: Option<Amount>) -> Transaction {
        Transaction { kind, client, tx, amount }
    }

    #[test]
    fn run_processes_transactions_via_ports() {
        let mut source = VecSource::new(vec![
            tx(TransactionKind::Deposit, 1, 1, Some(Amount::from_scaled(10_0000))),
            tx(TransactionKind::Withdrawal, 1, 2, Some(Amount::from_scaled(3_5000))),
            tx(TransactionKind::Dispute, 2, 10, None),
        ]);
        let mut sink = VecSink::new();

        run(&mut source, &mut sink).expect("run succeeds");

        let account = sink
            .accounts
            .iter()
            .find(|account| account.client == 1)
            .expect("client 1 account exists");
        assert_eq!(account.available, Amount::from_scaled(6_5000));
        assert_eq!(account.held, Amount::ZERO);
        assert!(!account.locked);
    }
}
