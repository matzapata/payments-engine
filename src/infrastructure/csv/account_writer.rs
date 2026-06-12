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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::account::Account;

    #[test]
    fn writes_four_decimal_amounts() {
        let mut output = Vec::new();
        let account = Account { client: 1, available: 10_000, held: 2345, locked: false };

        CsvAccountWriter::new(&mut output).write_accounts([account]).expect("write succeeds");

        let mut reader = csv::Reader::from_reader(output.as_slice());
        let headers = reader.headers().expect("headers").clone();
        assert_eq!(
            headers.iter().collect::<Vec<_>>(),
            ["client", "available", "held", "total", "locked"]
        );

        let row = reader.records().next().expect("data row").expect("valid row");
        assert_eq!(row.get(0).unwrap(), "1");
        assert_eq!(row.get(1).unwrap(), "1.0000");
        assert_eq!(row.get(2).unwrap(), "0.2345");
        assert_eq!(row.get(3).unwrap(), "1.2345");
        assert_eq!(row.get(4).unwrap(), "false");
    }
}
