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

    /// Seconds since 1970-01-01T00:00:00Z (the inverse of
    /// [`UtcDateTime::from_unix_seconds`]).
    pub fn unix_seconds(&self) -> i64 {
        // Days from the civil date (H. Hinnant's algorithm).
        let y = if self.month <= 2 {
            self.year - 1
        } else {
            self.year
        };
        let m = i64::from(self.month);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        let days = era * 146_097 + doe - 719_468;
        days * 86_400
            + i64::from(self.hour) * 3600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
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

/// Offset of this computer's local time from UTC, in seconds (east of UTC
/// positive), daylight saving included. Records stay in UTC; this is only
/// for times shown to the user.
pub fn local_offset_seconds() -> i64 {
    local::offset().unwrap_or(0)
}

/// A UTC moment as this computer's local clock time, for example
/// `Thu 08 Oct, 02:10`.
pub fn local_clock(unix_seconds: i64) -> String {
    let t = UtcDateTime::from_unix_seconds(unix_seconds + local_offset_seconds());
    let days = (unix_seconds + local_offset_seconds()).div_euclid(86_400);
    const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{} {:02} {}, {:02}:{:02}",
        WEEKDAYS[days.rem_euclid(7) as usize],
        t.day,
        MONTHS[(t.month as usize).clamp(1, 12) - 1],
        t.hour,
        t.minute
    )
}

/// Seconds since 1970 now.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(windows)]
mod local {
    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetLocalTime(t: *mut SystemTime);
        fn GetSystemTime(t: *mut SystemTime);
    }

    fn seconds(t: &SystemTime) -> i64 {
        super::UtcDateTime {
            year: i64::from(t.year),
            month: u32::from(t.month),
            day: u32::from(t.day),
            hour: u32::from(t.hour),
            minute: u32::from(t.minute),
            second: u32::from(t.second),
        }
        .unix_seconds()
    }

    pub fn offset() -> Option<i64> {
        let (mut l, mut u) = (SystemTime::default(), SystemTime::default());
        // SAFETY: both calls fill a SYSTEMTIME structure we own.
        unsafe {
            GetSystemTime(&mut u);
            GetLocalTime(&mut l);
        }
        // Round to whole minutes (the two calls are microseconds apart).
        let diff = seconds(&l) - seconds(&u);
        Some((diff as f64 / 60.0).round() as i64 * 60)
    }
}

#[cfg(unix)]
mod local {
    use std::ffi::c_char;

    /// `struct tm` of glibc, musl and macOS (64-bit): nine ints, then the
    /// offset from UTC and the zone name.
    #[repr(C)]
    struct Tm {
        sec: i32,
        min: i32,
        hour: i32,
        mday: i32,
        mon: i32,
        year: i32,
        wday: i32,
        yday: i32,
        isdst: i32,
        gmtoff: i64,
        zone: *const c_char,
    }

    extern "C" {
        fn localtime_r(time: *const i64, result: *mut Tm) -> *mut Tm;
    }

    pub fn offset() -> Option<i64> {
        let now = super::unix_now();
        let mut tm = Tm {
            sec: 0,
            min: 0,
            hour: 0,
            mday: 0,
            mon: 0,
            year: 0,
            wday: 0,
            yday: 0,
            isdst: 0,
            gmtoff: 0,
            zone: std::ptr::null(),
        };
        // SAFETY: localtime_r fills the structure we own and returns it.
        let r = unsafe { localtime_r(&now, &mut tm) };
        (!r.is_null()).then_some(tm.gmtoff)
    }
}

#[cfg(not(any(windows, unix)))]
mod local {
    pub fn offset() -> Option<i64> {
        None
    }
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

    #[test]
    fn unix_seconds_round_trip() {
        for secs in [0i64, 59, 86_399, 951_782_400, 1_791_312_204, 4_102_444_800] {
            assert_eq!(UtcDateTime::from_unix_seconds(secs).unix_seconds(), secs);
        }
    }

    #[test]
    fn local_clock_text_has_weekday_date_and_time() {
        // 1 January 1970 was a Thursday; the offset is whole minutes.
        assert_eq!(local_offset_seconds() % 60, 0);
        let text = local_clock(0 - local_offset_seconds());
        assert_eq!(text, "Thu 01 Jan, 00:00");
    }
}
