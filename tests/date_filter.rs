//! Tests for the local-time-zone date filter resolution.

use dcards::ui::widgets::{resolve, DatePreset};

/// Build a UTC timestamp from calendar components.
fn utc(year: i32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> i64 {
    chrono::NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, second)
        .unwrap()
        .and_utc()
        .timestamp()
}

#[test]
fn all_has_no_bounds() {
    assert_eq!(
        resolve(DatePreset::All, "", "", utc(2021, 6, 15, 12, 0, 0), 0),
        (None, None)
    );
}

#[test]
fn today_uses_local_midnight() {
    // UTC noon.
    let now = utc(2021, 1, 2, 12, 0, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 0),
        (Some(utc(2021, 1, 2, 0, 0, 0)), None)
    );

    // Just after local midnight (offset +2h).
    let now = utc(2021, 1, 2, 1, 0, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 2 * 3600),
        (Some(utc(2021, 1, 1, 22, 0, 0)), None)
    );
}

#[test]
fn exact_midnight_is_its_own_day() {
    let now = utc(2021, 1, 2, 0, 0, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 0),
        (Some(utc(2021, 1, 2, 0, 0, 0)), None)
    );
}

#[test]
fn last7_and_last30_span_local_days() {
    let now = utc(2021, 1, 2, 1, 0, 0);
    let offset = 2 * 3600;

    // 6 days before local 2021-01-02 -> 2020-12-27 local midnight.
    assert_eq!(
        resolve(DatePreset::Last7, "", "", now, offset),
        (Some(utc(2020, 12, 26, 22, 0, 0)), None)
    );

    // 29 days before local 2021-01-02 -> 2020-12-04 local midnight.
    assert_eq!(
        resolve(DatePreset::Last30, "", "", now, offset),
        (Some(utc(2020, 12, 3, 22, 0, 0)), None)
    );
}

#[test]
fn negative_offset_shifts_boundary_forward() {
    // 02:00 UTC on 2021-06-15 is 21:00 on 2021-06-14 in UTC-5.
    let now = utc(2021, 6, 15, 2, 0, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, -5 * 3600),
        (Some(utc(2021, 6, 14, 5, 0, 0)), None)
    );
}

#[test]
fn half_hour_offset() {
    // 20:00 UTC is 01:30 the next day at UTC+5:30.
    let now = utc(2021, 6, 15, 20, 0, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 5 * 3600 + 1800),
        (Some(utc(2021, 6, 15, 18, 30, 0)), None)
    );
}

#[test]
fn dst_offset_changes_the_boundary() {
    // The same instant near a DST switch, resolved with two offsets.
    let now = utc(2021, 3, 28, 0, 30, 0);
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 3600),
        (Some(utc(2021, 3, 27, 23, 0, 0)), None)
    );
    assert_eq!(
        resolve(DatePreset::Today, "", "", now, 7200),
        (Some(utc(2021, 3, 27, 22, 0, 0)), None)
    );
}

#[test]
fn custom_dates_are_local_and_inclusive() {
    let offset = 2 * 3600;
    let (from, to) = resolve(
        DatePreset::Custom,
        "2021-01-01",
        "2021-01-01",
        utc(2021, 6, 1, 0, 0, 0),
        offset,
    );
    // Local 2021-01-01 00:00 -> UTC 2020-12-31 22:00.
    assert_eq!(from, Some(utc(2020, 12, 31, 22, 0, 0)));
    // Local 2021-01-01 23:59:59 -> UTC 2021-01-01 21:59:59.
    assert_eq!(to, Some(utc(2021, 1, 1, 21, 59, 59)));
}

#[test]
fn custom_empty_or_invalid_fields_mean_no_bound() {
    let now = utc(2021, 6, 1, 0, 0, 0);
    assert_eq!(resolve(DatePreset::Custom, "", "", now, 0), (None, None));
    assert_eq!(
        resolve(DatePreset::Custom, "not-a-date", "2021-13-40", now, 0),
        (None, None)
    );
    assert_eq!(
        resolve(DatePreset::Custom, "2021-05-01", "garbage", now, 0),
        (Some(utc(2021, 5, 1, 0, 0, 0)), None)
    );
    assert_eq!(
        resolve(DatePreset::Custom, "", "2021-05-01", now, 0),
        (None, Some(utc(2021, 5, 1, 23, 59, 59)))
    );
}
