//! UTC timestamps without external dependencies.
//!
//! Run identifiers and log lines use Coordinated Universal Time, so they do not
//! depend on the computer's time zone settings.

use std::time::{SystemTime, UNIX_EPOCH};

/// A calendar date and time of day in UTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtcDateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl UtcDateTime {
    /// The current time, in whole seconds.
    pub fn now() -> Self {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        Self::from_unix_seconds(secs)
    }

    /// Converts seconds since 1970-01-01T00:00:00Z to a calendar date and time.
    pub fn from_unix_seconds(secs: i64) -> Self {
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400) as u32;
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: rem / 3600,
            minute: (rem % 3600) / 60,
            second: rem % 60,
        }
    }

    /// ISO 8601, for example `2026-10-02T05:36:01Z`.
    pub fn iso8601(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    /// Form used in run identifiers, for example `2026-10-02_053601`.
    pub fn run_id_part(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}_{:02}{:02}{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// Days since 1970-01-01 to (year, month, day) in the proleptic Gregorian
/// calendar (H. Hinnant's `civil_from_days` algorithm).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_and_known_dates() {
        assert_eq!(
            UtcDateTime::from_unix_seconds(0).iso8601(),
            "1970-01-01T00:00:00Z"
        );
        // 2000-02-29 (leap day) 12:34:56 UTC.
        assert_eq!(
            UtcDateTime::from_unix_seconds(951_827_696).iso8601(),
            "2000-02-29T12:34:56Z"
        );
        // 2026-10-02 05:36:01 UTC.
        let t = UtcDateTime::from_unix_seconds(1_790_919_361);
        assert_eq!(t.iso8601(), "2026-10-02T05:36:01Z");
        assert_eq!(t.run_id_part(), "2026-10-02_053601");
    }

    #[test]
    fn day_boundaries_are_continuous() {
        for days in -1000i64..1000 {
            let a = UtcDateTime::from_unix_seconds(days * 86_400 + 86_399);
            let b = UtcDateTime::from_unix_seconds((days + 1) * 86_400);
            assert_eq!((a.hour, a.minute, a.second), (23, 59, 59));
            assert_eq!((b.hour, b.minute, b.second), (0, 0, 0));
            assert!((b.year, b.month, b.day) > (a.year, a.month, a.day));
        }
    }
}
