mod cron;
mod systemd;

use std::env;
use std::fs;
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    let mut direction = "systemd".to_string();
    let mut input_path: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--to" => {
                i += 1;
                match args.get(i) {
                    Some(v) => direction = v.clone(),
                    None => {
                        eprintln!("--to requires a value (systemd|cron)");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => {
                if input_path.is_some() {
                    eprintln!("unexpected argument: {other}");
                    return ExitCode::FAILURE;
                }
                input_path = Some(other.to_string());
            }
        }
        i += 1;
    }

    let input = match read_input(input_path.as_deref()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading input: {e}");
            return ExitCode::FAILURE;
        }
    };

    let expr = input.trim();
    if expr.is_empty() {
        eprintln!("error: no cron expression given");
        return ExitCode::FAILURE;
    }

    match direction.as_str() {
        "systemd" => match cron::CronSchedule::parse(expr) {
            Ok(schedule) => match systemd::to_oncalendar(&schedule) {
                Ok(oncalendar) => {
                    println!("{oncalendar}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("error parsing cron expression: {e}");
                ExitCode::FAILURE
            }
        },
        "cron" => {
            eprintln!("error: systemd -> cron conversion is not implemented yet");
            ExitCode::FAILURE
        }
        other => {
            eprintln!("error: unknown --to target '{other}', expected 'systemd' or 'cron'");
            ExitCode::FAILURE
        }
    }
}

fn read_input(path: Option<&str>) -> io::Result<String> {
    match path {
        None | Some("-") => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
        Some(p) => fs::read_to_string(p),
    }
}

fn print_usage() {
    println!("cronvert - convert cron expressions to systemd OnCalendar syntax");
    println!();
    println!("usage:");
    println!("  cronvert [FILE]     read a 5-field cron expression from FILE");
    println!("  cronvert            read a 5-field cron expression from stdin");
    println!("  cronvert -          same as above, explicit stdin");
    println!("  cronvert --to systemd [FILE]   convert cron -> systemd OnCalendar (default)");
    println!();
    println!("examples:");
    println!("  echo '*/15 * * * *' | cronvert");
    println!("  cronvert my-crontab-line.txt");
}
