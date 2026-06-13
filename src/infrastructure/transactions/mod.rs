pub mod csv;
use crate::domain::transaction::Transaction;
pub use csv::CsvTransactionReader;

pub trait TransactionSource {
    type Error;
    fn next_transaction(&mut self) -> Option<Result<Transaction, Self::Error>>;
}
