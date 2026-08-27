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
        let dow_part = s
            .day_of_week
            .iter()
            .map(|d| WEEKDAY_NAMES[*d as usize])
            .collect::<Vec<_>>()
            .join(",");
        Ok(format!("{dow_part} {date_part} {time_part}"))
    } else {
        Ok(format!("{date_part} {time_part}"))
    }
}

fn field_to_systemd(values: &[u32], min: u32, max: u32) -> String {
    if is_full_range(values, min, max) {
        return "*".to_string();
    }
    values
        .iter()
        .map(|v| format!("{v:02}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn is_full_range(values: &[u32], min: u32, max: u32) -> bool {
    if values.len() != (max - min + 1) as usize {
        return false;
    }
    values.iter().enumerate().all(|(i, v)| *v == min + i as u32)
}
