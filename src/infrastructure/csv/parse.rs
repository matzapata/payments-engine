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
    let amount = row.amount.as_deref().and_then(parse_amount);

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
    let scaled = whole_part.checked_mul(10_000)?.checked_add(if negative {
        -fraction_part
    } else {
        fraction_part
    })?;

    Some(scaled)
}

pub(super) fn format_amount(amount: i64) -> String {
    let negative = amount.is_negative();
    let abs = amount.unsigned_abs();
    let whole = abs / 10_000;
    let fraction = abs % 10_000;
    if negative { format!("-{whole}.{fraction:04}") } else { format!("{whole}.{fraction:04}") }
}
