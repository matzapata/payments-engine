use crate::domain::account::Account;
pub mod csv;
pub use csv::CsvAccountWriter;

pub trait AccountSink {
    type Error;
    fn write_accounts(
        &mut self,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Result<(), Self::Error>;
}
