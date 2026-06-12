use std::io::Read;

use csv::StringRecord;

use crate::domain::transaction::Transaction;

use super::parse::parse_transaction_row;

pub struct CsvTransactionReader<R> {
    reader: csv::Reader<R>,
    headers: StringRecord,
}

impl<R: Read> CsvTransactionReader<R> {
    pub fn new(input: R) -> Result<Self, csv::Error> {
        let mut reader = csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(input);
        let headers = reader.headers()?.clone();
        Ok(Self { reader, headers })
    }

    pub fn transactions(&mut self) -> Transactions<'_, R> {
        Transactions { records: self.reader.records(), headers: &self.headers }
    }
}

pub struct Transactions<'a, R> {
    records: csv::StringRecordsIter<'a, R>,
    headers: &'a StringRecord,
}

impl<R: Read> Iterator for Transactions<'_, R> {
    type Item = Transaction;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let record = self.records.next()?;
            let record = record.ok()?;
            if let Some(transaction) = parse_transaction_row(self.headers, &record) {
                return Some(transaction);
            }
        }
    }
}
