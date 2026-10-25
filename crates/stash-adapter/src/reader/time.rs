//! Stash's `updated_at` text (spec 004 research R7, spec 005 research R5).

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// A Stash timestamp (`2026-10-21T10:00:00-06:00`) one second earlier, in the same text form and
/// offset. Stash compares `updated_at` filters as text to the second, so a catch-up threshold one
/// second early also catches items saved in the same second (003 findings).
pub fn one_second_earlier(stamp: &str) -> Option<String> {
    let (dt, offset) = stamp.split_at_checked(19)?;
    let n = |r: std::ops::Range<usize>| dt.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (h, mi, s) = (n(11..13)?, n(14..16)?, n(17..19)?);
    if dt.as_bytes().get(10) != Some(&b'T') || !(offset == "Z" || offset.len() == 6) {
        return None;
    }
    let secs = days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + s - 1;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    Some(format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}{offset}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    ))
}
