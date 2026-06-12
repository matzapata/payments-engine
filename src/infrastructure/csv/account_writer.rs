use std::io::Write;

use crate::domain::account::Account;

use super::parse::format_amount;

pub struct CsvAccountWriter<W> {
    output: W,
}

impl<W: Write> CsvAccountWriter<W> {
    pub fn new(output: W) -> Self {
        Self { output }
    }

    pub fn write_accounts(
        &mut self,
        accounts: impl IntoIterator<Item = Account>,
    ) -> Result<(), csv::Error> {
        let mut writer = csv::Writer::from_writer(&mut self.output);
        writer.write_record(["client", "available", "held", "total", "locked"])?;

        let mut rows: Vec<_> = accounts.into_iter().collect();
        rows.sort_by_key(|account| account.client);

        for account in rows {
            writer.write_record([
                account.client.to_string(),
                format_amount(account.available),
                format_amount(account.held),
                format_amount(account.total()),
                account.locked.to_string(),
            ])?;
        }

        writer.flush()?;
        Ok(())
    }
}
