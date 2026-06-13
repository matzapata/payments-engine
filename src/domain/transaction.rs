use std::fmt::{self, Display, Formatter};
use std::ops::{Add, Sub};
use std::str::FromStr;

/// Fixed-point money scaled ×10⁴ (four decimal places).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Amount(i64);

impl Amount {
    pub const ZERO: Self = Self(0);

    pub const fn from_scaled(value: i64) -> Self {
        Self(value)
    }

    pub const fn as_scaled(self) -> i64 {
        self.0
    }

    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self)
    }

    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Self)
    }
}

impl Add for Amount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Amount {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Display for Amount {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let negative = self.0.is_negative();
        let abs = self.0.unsigned_abs();
        let whole = abs / 10_000;
        let fraction = abs % 10_000;

        if negative {
            write!(f, "-{whole}.{fraction:04}")
        } else {
            write!(f, "{whole}.{fraction:04}")
        }
    }
}

impl FromStr for Amount {
    type Err = ParseAmountError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(ParseAmountError);
        }

        // Negative amounts are rejected at the parsing boundary: deposits and withdrawals
        // must be non-negative, and partner rows (dispute/resolve/chargeback) take their
        // amount from the original stored deposit, not from the row.
        if trimmed.starts_with('-') {
            return Err(ParseAmountError);
        }

        let (whole, fraction) = match trimmed.split_once('.') {
            Some((whole, fraction)) => (whole, fraction),
            None => (trimmed, ""),
        };

        if whole.is_empty() || fraction.len() > 4 || !whole.chars().all(|c| c.is_ascii_digit()) {
            return Err(ParseAmountError);
        }
        if !fraction.is_empty() && !fraction.chars().all(|c| c.is_ascii_digit()) {
            return Err(ParseAmountError);
        }

        let whole_part: i64 = whole.parse().map_err(|_| ParseAmountError)?;
        let fraction_part =
            format!("{fraction:0<4}")[..4].parse::<i64>().map_err(|_| ParseAmountError)?;
        let scaled = whole_part
            .checked_mul(10_000)
            .and_then(|value| value.checked_add(fraction_part))
            .ok_or(ParseAmountError)?;

        Ok(Self(scaled))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseAmountError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisputeState {
    None,
    Disputed,
    Resolved,
    ChargedBack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Amount>,
}
