# cronvert

Every time I move a job from crontab to a systemd timer I end up re-deriving
the `OnCalendar=` syntax by hand, usually by squinting at `man systemd.time`.
This is a small command-line tool that does that translation for me: give it
a standard 5-field cron expression and it prints the equivalent systemd
`OnCalendar` string.

## Usage

Read from stdin:

```
$ echo '*/15 * * * *' | cronvert
*-*-* *:00/15:00
```

Read from a file:

```
$ cat deploy.cron
30 2 * * 1-5
$ cronvert deploy.cron
Mon..Fri *-*-* 02:30:00
```

Read from stdin explicitly with `-`:

```
$ cronvert - < deploy.cron
```

Drop the output straight into a unit file:

```
[Timer]
OnCalendar=Mon..Fri *-*-* 02:30:00
```

## Supported cron syntax

- five whitespace-separated fields: minute, hour, day-of-month, month, day-of-week
- `*` (any value)
- exact values (`5`)
- ranges (`1-5`)
- lists (`1,3,5`)
- steps, on their own or on a range (`*/15`, `1-30/5`)
- month names (`JAN`-`DEC`) and weekday names (`SUN`-`SAT`), case-insensitive
- both `0` and `7` for Sunday in the day-of-week field

Output uses systemd's compact forms where they apply: contiguous runs become
`a..b`, evenly spaced runs become `a/step` (or `a..b/step` if they stop short
of the field's own max), and weekdays get named ranges like `Mon..Fri` — the
`OnCalendar` grammar has no step syntax for weekdays, so those are always
spelled out as a list.

## A known limitation

Cron treats day-of-month and day-of-week as an OR when both are restricted
(e.g. `0 9 1 * MON` fires on the 1st of the month *or* every Monday).
systemd's calendar spec ANDs its date and weekday fields instead, so that
kind of expression has no single `OnCalendar` equivalent. `cronvert` detects
this case and refuses to guess — it prints an error suggesting you split the
job into two timers.

## Building

No external dependencies, so a plain `cargo build --release` is enough. The
binary ends up at `target/release/cronvert`.

## Roadmap

See the roadmap notes in the project tracker; the short version is that
`--to cron` (systemd -> cron) and Quartz-style 6/7-field cron are not
supported yet.

## License

MIT, see [LICENSE](LICENSE).
