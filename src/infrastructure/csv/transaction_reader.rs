use crate::domain::transaction::Transaction;
use crate::domain::transaction::TransactionKind;
use csv::StringRecord;
use serde::Deserialize;
use std::io::Read;

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

#[derive(Debug, Deserialize)]
struct TransactionRow {
    #[serde(rename = "type")]
    kind: String,
    client: u16,
    tx: u32,
    amount: Option<String>,
}

fn parse_transaction_row(headers: &StringRecord, record: &StringRecord) -> Option<Transaction> {
    let row: TransactionRow = record.deserialize(Some(headers)).ok()?;
    let kind = parse_kind(&row.kind)?;
    let amount = match kind {
        TransactionKind::Deposit | TransactionKind::Withdrawal => {
            Some(row.amount.as_deref()?.parse().ok()?)
        }
        _ => row.amount.as_deref().and_then(|raw| raw.parse().ok()),
    };

    Some(Transaction { kind, client: row.client, tx: row.tx, amount })
}

fn parse_kind(raw: &str) -> Option<TransactionKind> {
    match raw.trim() {
        "deposit" => Some(TransactionKind::Deposit),
        "withdrawal" => Some(TransactionKind::Withdrawal),
        "dispute" => Some(TransactionKind::Dispute),
        "resolve" => Some(TransactionKind::Resolve),
        "chargeback" => Some(TransactionKind::Chargeback),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Amount;

    fn parse_row(line: &str) -> Option<Transaction> {
        let mut reader =
            csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(line.as_bytes());
        let headers = reader.headers().ok()?.clone();
        let record = reader.records().next()?.ok()?;
        parse_transaction_row(&headers, &record)
    }

    fn parse_csv(input: &str) -> Vec<Transaction> {
        let mut reader = CsvTransactionReader::new(input.as_bytes()).expect("valid csv");
        reader.transactions().collect()
    }

    #[test]
    fn whitespace_tolerant_fields() {
        let tx = parse_row("type,client,tx,amount\n deposit , 1 , 1 , 1.0 ").expect("parsed");
        assert_eq!(tx.kind, TransactionKind::Deposit);
        assert_eq!(tx.client, 1);
        assert_eq!(tx.tx, 1);
        assert_eq!(tx.amount, Some(Amount::from_scaled(10_000)));
    }

    #[test]
    fn one_point_zero_and_one_point_zero_zero_zero_zero_parse_equally() {
        let one = parse_row("type,client,tx,amount\ndeposit,1,1,1.0").expect("parsed 1.0");
        let four = parse_row("type,client,tx,amount\ndeposit,1,1,1.0000").expect("parsed 1.0000");
        assert_eq!(one.amount, Some(Amount::from_scaled(10_000)));
        assert_eq!(four.amount, Some(Amount::from_scaled(10_000)));
    }

    #[test]
    fn invalid_type_is_skipped() {
        assert!(parse_row("type,client,tx,amount\nfoo,1,1,1.0").is_none());
    }

    #[test]
    fn bad_client_is_skipped() {
        assert!(parse_row("type,client,tx,amount\ndeposit,abc,1,1.0").is_none());
    }

    #[test]
    fn bad_deposit_amounts_are_skipped() {
        for line in [
            "type,client,tx,amount\ndeposit,1,1,",
            "type,client,tx,amount\ndeposit,1,1,abc",
            "type,client,tx,amount\ndeposit,1,1,1.12345",
        ] {
            assert!(parse_row(line).is_none(), "expected skip for {line}");
        }
    }

    #[test]
    fn bad_withdrawal_amounts_are_skipped() {
        for line in [
            "type,client,tx,amount\nwithdrawal,1,2,",
            "type,client,tx,amount\nwithdrawal,1,2,abc",
            "type,client,tx,amount\nwithdrawal,1,2,1.12345",
        ] {
            assert!(parse_row(line).is_none(), "expected skip for {line}");
        }
    }

    #[test]
    fn deposit_without_parseable_amount_is_skipped() {
        let csv = "type,client,tx,amount\n\
                   deposit,1,1,\n\
                   deposit,2,2,abc\n\
                   deposit,3,3,1.0\n";
        let transactions = parse_csv(csv);
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].client, 3);
        assert_eq!(transactions[0].amount, Some(Amount::from_scaled(10_000)));
    }

    #[test]
    fn withdrawal_without_parseable_amount_is_skipped() {
        let csv = "type,client,tx,amount\n\
                   withdrawal,1,1,\n\
                   withdrawal,2,2,abc\n\
                   withdrawal,3,3,1.0\n";
        let transactions = parse_csv(csv);
        assert_eq!(transactions.len(), 1);
        assert_eq!(transactions[0].client, 3);
        assert_eq!(transactions[0].amount, Some(Amount::from_scaled(10_000)));
    }

    #[test]
    fn partner_transaction_without_amount_is_parsed() {
        for (kind_str, kind) in [
            ("dispute", TransactionKind::Dispute),
            ("resolve", TransactionKind::Resolve),
            ("chargeback", TransactionKind::Chargeback),
        ] {
            let line = format!("type,client,tx,amount\n{kind_str},1,1,");
            let tx = parse_row(&line).unwrap_or_else(|| panic!("expected {kind_str} to parse"));
            assert_eq!(tx.kind, kind, "{kind_str}");
            assert_eq!(tx.amount, None, "{kind_str}");
        }
    }

    #[test]
    fn amount_parsing_supports_negatives() {
        assert_eq!("-1.2345".parse::<Amount>().ok(), Some(Amount::from_scaled(-12_345)));
        assert_eq!("-0.0001".parse::<Amount>().ok(), Some(Amount::from_scaled(-1)));
    }

    #[test]
    fn amount_display_formats_zero() {
        assert_eq!(Amount::ZERO.to_string(), "0.0000");
    }

    #[test]
    fn amount_display_formats_whole_number() {
        assert_eq!(Amount::from_scaled(10_000).to_string(), "1.0000");
    }

    #[test]
    fn amount_display_formats_fractional() {
        assert_eq!(Amount::from_scaled(12_3450).to_string(), "12.3450");
    }

    #[test]
    fn amount_display_formats_negative() {
        assert_eq!(Amount::from_scaled(-12_345).to_string(), "-1.2345");
    }

    #[test]
    fn amount_display_zero_pads_fraction() {
        assert_eq!(Amount::from_scaled(100_0000).to_string(), "100.0000");
    }
}
