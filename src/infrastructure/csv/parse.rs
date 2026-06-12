use csv::StringRecord;
use serde::Deserialize;

use crate::domain::transaction::{Transaction, TransactionKind};

#[derive(Debug, Deserialize)]
pub(super) struct TransactionRow {
    #[serde(rename = "type")]
    kind: String,
    client: u16,
    tx: u32,
    amount: Option<String>,
}

pub(super) fn parse_transaction_row(
    headers: &StringRecord,
    record: &StringRecord,
) -> Option<Transaction> {
    let row: TransactionRow = record.deserialize(Some(headers)).ok()?;
    let kind = parse_kind(&row.kind)?;
    let amount = match kind {
        TransactionKind::Deposit | TransactionKind::Withdrawal => {
            Some(row.amount.as_deref().and_then(parse_amount)?)
        }
        _ => row.amount.as_deref().and_then(parse_amount),
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

fn parse_amount(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let negative = trimmed.starts_with('-');
    let digits = trimmed.trim_start_matches('-');
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (digits, ""),
    };

    if whole.is_empty() || fraction.len() > 4 || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !fraction.is_empty() && !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let whole_part: i64 = whole.parse().ok()?;
    let fraction_part = format!("{fraction:0<4}")[..4].parse::<i64>().ok()?;
    let abs_scaled = whole_part.checked_mul(10_000)?.checked_add(fraction_part)?;
    if negative { abs_scaled.checked_neg() } else { Some(abs_scaled) }
}

pub(super) fn format_amount(amount: i64) -> String {
    let negative = amount.is_negative();
    let abs = amount.unsigned_abs();
    let whole = abs / 10_000;
    let fraction = abs % 10_000;
    if negative { format!("-{whole}.{fraction:04}") } else { format!("{whole}.{fraction:04}") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::transaction::TransactionKind;

    fn parse_row(line: &str) -> Option<Transaction> {
        let mut reader =
            csv::ReaderBuilder::new().trim(csv::Trim::All).from_reader(line.as_bytes());
        let headers = reader.headers().ok()?.clone();
        let record = reader.records().next()?.ok()?;
        parse_transaction_row(&headers, &record)
    }

    fn parse_csv(input: &str) -> Vec<Transaction> {
        use super::super::transaction_reader::CsvTransactionReader;

        let mut reader = CsvTransactionReader::new(input.as_bytes()).expect("valid csv");
        reader.transactions().collect()
    }

    #[test]
    fn whitespace_tolerant_fields() {
        let tx = parse_row("type,client,tx,amount\n deposit , 1 , 1 , 1.0 ").expect("parsed");
        assert_eq!(tx.kind, TransactionKind::Deposit);
        assert_eq!(tx.client, 1);
        assert_eq!(tx.tx, 1);
        assert_eq!(tx.amount, Some(10_000));
    }

    #[test]
    fn one_point_zero_and_one_point_zero_zero_zero_zero_parse_equally() {
        let one = parse_row("type,client,tx,amount\ndeposit,1,1,1.0").expect("parsed 1.0");
        let four = parse_row("type,client,tx,amount\ndeposit,1,1,1.0000").expect("parsed 1.0000");
        assert_eq!(one.amount, Some(10_000));
        assert_eq!(four.amount, Some(10_000));
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
        assert_eq!(transactions[0].amount, Some(10_000));
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
        assert_eq!(transactions[0].amount, Some(10_000));
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
    fn negative_amounts_parse_correctly() {
        assert_eq!(parse_amount("-1.2345"), Some(-12_345));
        assert_eq!(parse_amount("-0.0001"), Some(-1));
    }

    #[test]
    fn format_amount_zero() {
        assert_eq!(format_amount(0), "0.0000");
    }

    #[test]
    fn format_amount_whole_number() {
        assert_eq!(format_amount(10_000), "1.0000");
    }

    #[test]
    fn format_amount_fractional() {
        assert_eq!(format_amount(12_3450), "12.3450");
    }

    #[test]
    fn format_amount_negative() {
        assert_eq!(format_amount(-12_345), "-1.2345");
    }

    #[test]
    fn format_amount_zero_pads_fraction() {
        assert_eq!(format_amount(100_0000), "100.0000");
    }
}
