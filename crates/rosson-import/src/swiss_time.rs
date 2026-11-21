//! Rosson keeps dates and times of day as JavaScript dates, serialised in UTC: a class on
//! 25 September is `2026-09-25T00:00:00.000Z`, a start at 08:30 is `1970-01-01T07:30:00.000Z`
//! (a time on 1 January 1970, Swiss winter time). These read them back as Swiss local dates and
//! times.

use show::model::TimeOfDay;

/// `YYYY-MM-DDTHH:MM:SS(.sss)Z` → (year, month, day, minutes after midnight), UTC.
fn parse(iso: &str) -> Option<(i32, u32, u32, i64)> {
    let (date, time) = iso.split_once('T').unwrap_or((iso, "00:00:00Z"));
    let mut d = date.split('-').map(|p| p.parse::<i64>());
    let (y, m, day) = (d.next()?.ok()?, d.next()?.ok()?, d.next()?.ok()?);
    let mut t = time.trim_end_matches('Z').split(':').map(|p| p.split('.').next().unwrap_or("0").parse::<i64>());
    let (h, mi) = (t.next().and_then(Result::ok).unwrap_or(0), t.next().and_then(Result::ok).unwrap_or(0));
    Some((y as i32, m as u32, day as u32, h * 60 + mi))
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Day of week, 0 = Sunday (Zeller-like, Gregorian).
fn weekday(y: i32, m: u32, d: u32) -> u32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if m < 3 { y - 1 } else { y };
    ((y + y / 4 - y / 100 + y / 400 + t[(m - 1) as usize] + d as i32) % 7) as u32
}

fn last_sunday(y: i32, m: u32) -> u32 {
    let last = days_in_month(y, m);
    last - weekday(y, m, last)
}

/// Swiss offset from UTC in minutes at a UTC instant: summer time (since 1981) from the last
/// Sunday of March 01:00 UTC to the last Sunday of October 01:00 UTC.
fn swiss_offset(y: i32, m: u32, d: u32, minutes: i64) -> i64 {
    if y < 1981 {
        return 60;
    }
    let key = |m: u32, d: u32, min: i64| (m, d, min);
    let start = key(3, last_sunday(y, 3), 60);
    let end = key(10, last_sunday(y, 10), 60);
    if (m, d, minutes) >= start && (m, d, minutes) < end { 120 } else { 60 }
}

/// The Swiss calendar date of a Rosson date, `YYYY-MM-DD`.
pub fn date(iso: &str) -> Option<String> {
    let (mut y, mut m, mut d, minutes) = parse(iso)?;
    if minutes + swiss_offset(y, m, d, minutes) >= 24 * 60 {
        d += 1;
        if d > days_in_month(y, m) {
            d = 1;
            m += 1;
            if m > 12 {
                m = 1;
                y += 1;
            }
        }
    }
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// The Swiss time of day of a Rosson time.
pub fn time_of_day(iso: &str) -> Option<TimeOfDay> {
    let (y, m, d, minutes) = parse(iso)?;
    Some(TimeOfDay(((minutes + swiss_offset(y, m, d, minutes)).rem_euclid(24 * 60)) as u16))
}

/// The Swiss time of day of a Rosson date-time, in seconds since midnight (with the
/// milliseconds): a marathon's start or finish.
pub fn seconds_of_day(iso: &str) -> Option<rust_decimal::Decimal> {
    let (y, m, d, minutes) = parse(iso)?;
    let rest = iso.get(17..)?.trim_end_matches('Z');
    let seconds: rust_decimal::Decimal = rest.parse().ok()?;
    let local = (minutes + swiss_offset(y, m, d, minutes)).rem_euclid(24 * 60);
    Some((rust_decimal::Decimal::from(local * 60) + seconds).normalize())
}

/// A Swiss time of day (seconds since midnight) on a date, as Rosson keeps a date-time (UTC).
pub fn iso_at(date: &str, seconds: rust_decimal::Decimal) -> String {
    let mut parts = date.get(..10).unwrap_or(date).split('-').map(|p| p.parse::<i32>().unwrap_or(0));
    let (y, m, d) = (parts.next().unwrap_or(1970), parts.next().unwrap_or(1) as u32, parts.next().unwrap_or(1) as u32);
    let whole = seconds.trunc().to_string().parse::<i64>().unwrap_or(0);
    let millis = ((seconds.fract()) * rust_decimal::Decimal::from(1000)).round().to_string().parse::<i64>().unwrap_or(0);
    let offset = swiss_offset(y, m, d, whole / 60 - 60) * 60;
    let utc = (whole - offset).rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{millis:03}Z", utc / 3600, utc / 60 % 60, utc % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_rosson_dates_and_times() {
        assert_eq!(date("2026-09-25T00:00:00.000Z").as_deref(), Some("2026-09-25"));
        assert_eq!(date("2026-09-24T22:00:00.000Z").as_deref(), Some("2026-09-25")); // local midnight, summer
        assert_eq!(date("2026-12-24T23:00:00.000Z").as_deref(), Some("2026-12-25")); // winter
        assert_eq!(date("2026-09-25").as_deref(), Some("2026-09-25"));
        assert_eq!(time_of_day("1970-01-01T07:30:00.000Z"), Some(TimeOfDay::new(8, 30)));
        assert_eq!(time_of_day("1970-01-01T23:30:00.000Z"), Some(TimeOfDay::new(0, 30)));
        assert_eq!(last_sunday(2026, 3), 29);
        assert_eq!(last_sunday(2026, 10), 25);
        // A marathon's finish in summer: 10:30:10.5 Swiss time.
        assert_eq!(seconds_of_day("2026-09-27T08:30:10.500Z"), Some(rust_decimal::Decimal::new(378_105, 1)));
        assert_eq!(iso_at("2026-09-27", rust_decimal::Decimal::new(378_105, 1)), "2026-09-27T08:30:10.500Z");
    }
}
