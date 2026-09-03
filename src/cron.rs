// Parses standard 5-field Unix cron expressions (minute hour dom month dow)
// into an explicit set of matching values per field.

use std::collections::BTreeSet;

pub struct CronSchedule {
    pub minute: Vec<u32>,
    pub hour: Vec<u32>,
    pub day_of_month: Vec<u32>,
    pub month: Vec<u32>,
    pub day_of_week: Vec<u32>, // normalized: 0 = Sunday ... 6 = Saturday
    // cron's dom/dow combination is OR'd when both are restricted, which most
    // target formats can't express directly. Track this so the converter can
    // refuse instead of silently producing a wrong schedule.
    pub dom_restricted: bool,
    pub dow_restricted: bool,
}

const MONTH_NAMES: &[(&str, u32)] = &[
    ("JAN", 1), ("FEB", 2), ("MAR", 3), ("APR", 4),
    ("MAY", 5), ("JUN", 6), ("JUL", 7), ("AUG", 8),
    ("SEP", 9), ("OCT", 10), ("NOV", 11), ("DEC", 12),
];

const DOW_NAMES: &[(&str, u32)] = &[
    ("SUN", 0), ("MON", 1), ("TUE", 2), ("WED", 3),
    ("THU", 4), ("FRI", 5), ("SAT", 6),
];

impl CronSchedule {
    pub fn parse(expr: &str) -> Result<CronSchedule, String> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(format!(
                "expected 5 fields (minute hour day-of-month month day-of-week), got {}: '{}'",
                fields.len(),
                expr
            ));
        }

        let minute = parse_field(fields[0], 0, 59, &[])?;
        let hour = parse_field(fields[1], 0, 23, &[])?;
        let day_of_month = parse_field(fields[2], 1, 31, &[])?;
        let month = parse_field(fields[3], 1, 12, MONTH_NAMES)?;

        // day-of-week accepts 0-7, where both 0 and 7 mean Sunday
        let mut day_of_week = parse_field(fields[4], 0, 7, DOW_NAMES)?;
        for v in day_of_week.iter_mut() {
            if *v == 7 {
                *v = 0;
            }
        }
        day_of_week.sort_unstable();
        day_of_week.dedup();

        let dom_restricted = fields[2].trim() != "*";
        let dow_restricted = fields[4].trim() != "*";

        Ok(CronSchedule {
            minute,
            hour,
            day_of_month,
            month,
            day_of_week,
            dom_restricted,
            dow_restricted,
        })
    }
}

fn parse_field(spec: &str, min: u32, max: u32, names: &[(&str, u32)]) -> Result<Vec<u32>, String> {
    let mut values = BTreeSet::new();

    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("empty field component in '{spec}'"));
        }

        let (range_part, step) = match part.split_once('/') {
            Some((r, s)) => {
                let step: u32 = s
                    .parse()
                    .map_err(|_| format!("invalid step '{s}' in '{part}'"))?;
                if step == 0 {
                    return Err(format!("step cannot be zero in '{part}'"));
                }
                (r, step)
            }
            None => (part, 1),
        };

        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            (resolve_value(a, names)?, resolve_value(b, names)?)
        } else {
            let v = resolve_value(range_part, names)?;
            (v, v)
        };

        if start < min || end > max || start > end {
            return Err(format!(
                "value out of range in '{part}' (expected {min}-{max})"
            ));
        }

        let mut v = start;
        while v <= end {
            values.insert(v);
            v += step;
        }
    }

    Ok(values.into_iter().collect())
}

fn resolve_value(token: &str, names: &[(&str, u32)]) -> Result<u32, String> {
    let upper = token.to_ascii_uppercase();
    for (name, value) in names {
        if *name == upper {
            return Ok(*value);
        }
    }
    token
        .parse::<u32>()
        .map_err(|_| format!("invalid value '{token}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_value() {
        assert_eq!(parse_field("5", 0, 59, &[]).unwrap(), vec![5]);
    }

    #[test]
    fn wildcard_covers_full_range() {
        assert_eq!(parse_field("*", 0, 3, &[]).unwrap(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn range_is_inclusive() {
        assert_eq!(parse_field("1-5", 0, 59, &[]).unwrap(), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn list_is_sorted_and_deduped() {
        assert_eq!(parse_field("5,1,3,1", 0, 59, &[]).unwrap(), vec![1, 3, 5]);
    }

    #[test]
    fn step_on_wildcard() {
        assert_eq!(parse_field("*/15", 0, 59, &[]).unwrap(), vec![0, 15, 30, 45]);
    }

    #[test]
    fn step_on_range() {
        assert_eq!(parse_field("1-10/3", 0, 59, &[]).unwrap(), vec![1, 4, 7, 10]);
    }

    #[test]
    fn step_does_not_overshoot_range_end() {
        // a step that lands past the range's upper bound must not be included
        assert_eq!(parse_field("0-10/4", 0, 59, &[]).unwrap(), vec![0, 4, 8]);
    }

    #[test]
    fn named_month_is_case_insensitive() {
        assert_eq!(parse_field("jan", 1, 12, MONTH_NAMES).unwrap(), vec![1]);
        assert_eq!(parse_field("DEC", 1, 12, MONTH_NAMES).unwrap(), vec![12]);
    }

    #[test]
    fn named_range_resolves_both_ends() {
        assert_eq!(
            parse_field("MON-FRI", 0, 7, DOW_NAMES).unwrap(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn rejects_zero_step() {
        assert!(parse_field("*/0", 0, 59, &[]).is_err());
    }

    #[test]
    fn rejects_out_of_range_value() {
        assert!(parse_field("60", 0, 59, &[]).is_err());
        assert!(parse_field("-1", 0, 59, &[]).is_err());
    }

    #[test]
    fn rejects_inverted_range() {
        assert!(parse_field("5-3", 0, 59, &[]).is_err());
    }

    #[test]
    fn rejects_empty_list_component() {
        assert!(parse_field("1,,3", 0, 59, &[]).is_err());
    }

    #[test]
    fn rejects_unknown_name() {
        assert!(parse_field("FOO", 1, 12, MONTH_NAMES).is_err());
    }

    #[test]
    fn schedule_requires_five_fields() {
        assert!(CronSchedule::parse("* * * *").is_err());
        assert!(CronSchedule::parse("* * * * * *").is_err());
    }

    #[test]
    fn schedule_normalizes_sunday_seven_to_zero() {
        let s = CronSchedule::parse("0 0 * * 0,7").unwrap();
        assert_eq!(s.day_of_week, vec![0]);
    }

    #[test]
    fn schedule_tracks_dom_and_dow_restriction() {
        let unrestricted = CronSchedule::parse("* * * * *").unwrap();
        assert!(!unrestricted.dom_restricted);
        assert!(!unrestricted.dow_restricted);

        let both_restricted = CronSchedule::parse("0 9 1 * MON").unwrap();
        assert!(both_restricted.dom_restricted);
        assert!(both_restricted.dow_restricted);

        let dow_only = CronSchedule::parse("0 9 * * MON").unwrap();
        assert!(!dow_only.dom_restricted);
        assert!(dow_only.dow_restricted);
    }
}
