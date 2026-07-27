//! Clock — injectable time for tests and budgets.

use chrono::{DateTime, Utc};
use std::time::Duration;

pub trait Clock {
    fn now_utc(&self) -> DateTime<Utc>;
    fn sleep(&self, d: Duration);
}
