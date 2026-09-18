// Renders a parsed CronSchedule as a systemd.time(7) OnCalendar expression.

use crate::cron::CronSchedule;

const WEEKDAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub fn to_oncalendar(s: &CronSchedule) -> Result<String, String> {
    if s.dom_restricted && s.dow_restricted {
        return Err(
            "cron ORs a restricted day-of-month with a restricted day-of-week (it fires when \
             either matches), but systemd ANDs its date and weekday fields. This schedule can't \
             be expressed as one OnCalendar expression \
             — split it into two systemd timers instead."
                .to_string(),
        );
    }

    let hour_part = field_to_systemd(&s.hour, 0, 23);
    let minute_part = field_to_systemd(&s.minute, 0, 59);
    let time_part = format!("{hour_part}:{minute_part}:00");

    let month_part = field_to_systemd(&s.month, 1, 12);
    let day_part = field_to_systemd(&s.day_of_month, 1, 31);
    let date_part = format!("*-{month_part}-{day_part}");

    if s.dow_restricted {
        let dow_part = render_weekdays(&s.day_of_week);
        Ok(format!("{dow_part} {date_part} {time_part}"))
    } else {
        Ok(format!("{date_part} {time_part}"))
    }
}

// A contiguous or evenly-spaced run of values, ready to render either as
// systemd's numeric "a..b" / "a/step" / "a..b/step" syntax or, for weekdays,
// as names. `end: None` on a step means the run isn't capped — it repeats up
// to the field's own upper bound, matching systemd's "start/step" shorthand.
enum Group {
    Single(u32),
    Range(u32, u32),
    Step { start: u32, end: Option<u32>, step: u32 },
}

// Greedily walks the sorted, deduped values and folds runs of three or more
// into a single Group. Shorter runs aren't worth compacting: "1,2" is no
// longer than "1..2".
fn group_values(values: &[u32], max: u32) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < values.len() {
        let mut j = i;
        let mut step = 0;
        while j + 1 < values.len() {
            let diff = values[j + 1] - values[j];
            if j == i {
                step = diff;
            } else if diff != step {
                break;
            }
            j += 1;
        }

        let run_len = j - i + 1;
        if run_len >= 3 {
            let start = values[i];
            let end = values[j];
            if step == 1 {
                groups.push(Group::Range(start, end));
            } else if end + step > max {
                groups.push(Group::Step { start, end: None, step });
            } else {
                groups.push(Group::Step { start, end: Some(end), step });
            }
            i = j + 1;
        } else {
            groups.push(Group::Single(values[i]));
            i += 1;
        }
    }
    groups
}

fn field_to_systemd(values: &[u32], min: u32, max: u32) -> String {
    if is_full_range(values, min, max) {
        return "*".to_string();
    }
    group_values(values, max)
        .iter()
        .map(|g| match g {
            Group::Single(v) => format!("{v:02}"),
            Group::Range(s, e) => format!("{s:02}..{e:02}"),
            Group::Step { start, end: None, step } => format!("{start:02}/{step}"),
            Group::Step { start, end: Some(end), step } => format!("{start:02}..{end:02}/{step}"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

// systemd's weekday grammar only accepts single names or "Name..Name" ranges
// — no step syntax — so unlike group_values, non-consecutive runs are never
// folded into a step here, even when they're evenly spaced.
fn group_weekdays(values: &[u32]) -> Vec<Group> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < values.len() {
        let mut j = i;
        while j + 1 < values.len() && values[j + 1] - values[j] == 1 {
            j += 1;
        }
        if j - i + 1 >= 3 {
            groups.push(Group::Range(values[i], values[j]));
        } else {
            for v in &values[i..=j] {
                groups.push(Group::Single(*v));
            }
        }
        i = j + 1;
    }
    groups
}

fn render_weekdays(values: &[u32]) -> String {
    group_weekdays(values)
        .iter()
        .map(|g| match g {
            Group::Single(v) => WEEKDAY_NAMES[*v as usize].to_string(),
            Group::Range(s, e) => {
                format!("{}..{}", WEEKDAY_NAMES[*s as usize], WEEKDAY_NAMES[*e as usize])
            }
            Group::Step { .. } => unreachable!("group_weekdays never produces a Step"),
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn is_full_range(values: &[u32], min: u32, max: u32) -> bool {
    if values.len() != (max - min + 1) as usize {
        return false;
    }
    values.iter().enumerate().all(|(i, v)| *v == min + i as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cron::CronSchedule;

    #[test]
    fn wildcard_field_renders_as_star() {
        assert_eq!(field_to_systemd(&[0, 1, 2, 3], 0, 3), "*");
    }

    #[test]
    fn short_list_is_not_compacted() {
        assert_eq!(field_to_systemd(&[1, 2], 0, 59), "01,02");
    }

    #[test]
    fn contiguous_run_becomes_a_range() {
        assert_eq!(field_to_systemd(&[1, 2, 3, 4, 5], 0, 59), "01..05");
    }

    #[test]
    fn step_that_reaches_the_field_max_is_unbounded() {
        assert_eq!(field_to_systemd(&[0, 15, 30, 45], 0, 59), "00/15");
    }

    #[test]
    fn step_that_stops_short_of_max_keeps_its_end() {
        assert_eq!(field_to_systemd(&[1, 4, 7, 10], 0, 59), "01..10/3");
    }

    #[test]
    fn mixed_groups_are_joined_with_commas() {
        assert_eq!(field_to_systemd(&[1, 2, 3, 10, 20, 35], 0, 59), "01..03,10,20,35");
    }

    #[test]
    fn three_evenly_spaced_values_compact_to_a_bounded_step() {
        assert_eq!(field_to_systemd(&[10, 20, 30], 0, 59), "10..30/10");
    }

    #[test]
    fn weekday_range_uses_names_not_numbers() {
        assert_eq!(render_weekdays(&[1, 2, 3, 4, 5]), "Mon..Fri");
    }

    #[test]
    fn weekday_step_is_never_compacted() {
        // systemd's weekday grammar has no step syntax, unlike numeric fields.
        assert_eq!(render_weekdays(&[0, 2, 4, 6]), "Sun,Tue,Thu,Sat");
    }

    #[test]
    fn full_conversion_uses_compact_syntax() {
        let s = CronSchedule::parse("*/15 * * * *").unwrap();
        assert_eq!(to_oncalendar(&s).unwrap(), "*-*-* *:00/15:00");
    }

    #[test]
    fn full_conversion_compacts_weekday_range() {
        let s = CronSchedule::parse("30 2 * * 1-5").unwrap();
        assert_eq!(to_oncalendar(&s).unwrap(), "Mon..Fri *-*-* 02:30:00");
    }
}
