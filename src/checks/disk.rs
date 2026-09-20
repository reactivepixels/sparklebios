//! Disk filling trend: not how full the disk is, but how fast it is getting there.

use serde::{Deserialize, Serialize};

use super::{plural, Finding, Severity};

const FINDING_TTL: u64 = 86400;
/// Readings are kept for a fortnight, which is long enough to see a trend and short enough that
/// a clear-out a week ago stops dragging the answer down.
const KEEP_READINGS: usize = 14;
/// Below three readings there is no trend, only two points and a straight line through them.
const MIN_READINGS: usize = 3;
/// Only worth saying when it is close enough to act on.
const WARN_WITHIN_DAYS: u64 = 30;

/// One day's free space, in whole GB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reading {
    pub at: u64,
    pub free_gb: u64,
}

/// `history` with today's reading in it: one per day, the most recent replacing today's earlier
/// one, oldest dropped past `KEEP_READINGS`. Unchanged when the free space cannot be read.
pub(crate) fn record(mut history: Vec<Reading>, now: u64, free_gb: Option<u64>) -> Vec<Reading> {
    let Some(free_gb) = free_gb else {
        return history;
    };
    let today = now / 86_400;
    match history.last_mut() {
        Some(last) if last.at / 86_400 == today => {
            last.free_gb = free_gb;
            last.at = now;
        }
        _ => history.push(Reading { at: now, free_gb }),
    }
    // A clock that jumped backwards would otherwise leave readings out of order forever.
    history.retain(|r| r.at <= now);
    let overflow = history.len().saturating_sub(KEEP_READINGS);
    history.drain(..overflow);
    history
}

pub(crate) fn check(history: &[Reading]) -> Option<Finding> {
    let (days_left, rate) = projection(history)?;
    if days_left > WARN_WITHIN_DAYS {
        return None;
    }
    let mut facts = std::collections::BTreeMap::new();
    facts.insert(
        "disk.days_left".to_string(),
        plural(days_left, "day", "days"),
    );
    facts.insert("disk.rate".to_string(), format!("{rate:.1}GB a day"));
    Some(Finding {
        id: "disk_trend".to_string(),
        severity: Severity::Warn,
        ttl: FINDING_TTL,
        facts,
    })
}

/// Days until the disk is full at the rate it has been filling, and that rate in GB a day.
/// `None` when there are too few readings, when they do not span a day, or when the disk is not
/// filling at all, which is the usual answer and the reason this check is quiet most of the time.
fn projection(history: &[Reading]) -> Option<(u64, f64)> {
    if history.len() < MIN_READINGS {
        return None;
    }
    let first = history.first()?;
    let last = history.last()?;
    let span_days = (last.at.checked_sub(first.at)? as f64) / 86_400.0;
    if span_days < 1.0 {
        return None;
    }
    let lost_gb = (first.free_gb as f64) - (last.free_gb as f64);
    let rate = lost_gb / span_days;
    if rate <= 0.0 {
        return None;
    }
    // Rounded up, and never below one: less than a day away still reads as "1 day", where
    // "0 days" reads as a broken counter rather than an urgent one.
    let days_left = ((last.free_gb as f64) / rate).ceil().max(1.0);
    // A rate so slow the answer overflows is the same as not filling.
    days_left.is_finite().then_some((days_left as u64, rate))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400;

    fn series(free: &[u64]) -> Vec<Reading> {
        free.iter()
            .enumerate()
            .map(|(i, &free_gb)| Reading {
                at: (i as u64 + 1) * DAY,
                free_gb,
            })
            .collect()
    }

    #[test]
    fn fewer_than_three_readings_is_not_a_trend() {
        assert!(check(&series(&[100, 50])).is_none());
        assert!(projection(&series(&[100, 50])).is_none());
    }

    #[test]
    fn a_disk_that_is_not_filling_says_nothing() {
        assert!(check(&series(&[100, 100, 100])).is_none());
        assert!(check(&series(&[100, 110, 120])).is_none());
    }

    #[test]
    fn a_disk_filling_slowly_is_still_too_far_off_to_mention() {
        // one GB a day with 500 left is well over a month away
        assert!(check(&series(&[503, 502, 501, 500])).is_none());
    }

    #[test]
    fn a_disk_filling_fast_enough_to_matter_reports_when_and_how_fast() {
        // ten GB a day with 40 left: four days
        let f = check(&series(&[70, 60, 50, 40])).unwrap();
        assert_eq!(f.id, "disk_trend");
        assert_eq!(f.severity, Severity::Warn);
        assert_eq!(f.facts.get("disk.days_left").unwrap(), "4 days");
        assert_eq!(f.facts.get("disk.rate").unwrap(), "10.0GB a day");
    }

    #[test]
    fn a_disk_hours_from_full_still_reads_as_a_day_rather_than_zero() {
        // losing 40GB a day with 19 left is a third of a day away
        let f = check(&series(&[180, 140, 100, 19])).unwrap();
        assert_eq!(f.facts.get("disk.days_left").unwrap(), "1 day");
    }

    #[test]
    fn one_reading_a_day_is_kept_and_the_oldest_fall_off() {
        let mut history = Vec::new();
        for day in 1..=20u64 {
            history = record(history, day * DAY, Some(100 - day));
        }
        assert_eq!(history.len(), KEEP_READINGS);
        assert_eq!(history.last().unwrap().free_gb, 80);
    }

    #[test]
    fn a_second_reading_the_same_day_replaces_the_first() {
        let history = record(Vec::new(), DAY, Some(100));
        let history = record(history, DAY + 3600, Some(90));
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].free_gb, 90);
    }

    #[test]
    fn an_unreadable_disk_leaves_the_history_alone() {
        let history = record(Vec::new(), DAY, Some(100));
        let history = record(history, 2 * DAY, None);
        assert_eq!(history.len(), 1);
    }
}
