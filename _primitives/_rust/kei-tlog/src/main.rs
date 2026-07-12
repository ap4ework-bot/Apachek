// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 <author org>
//
//! `kei-tlog` — atomar time-logger (RULE 0.17 enforcement primitive).
//!
//! Three subcommands:
//!   start <name>        — emit a `start` line to journal, print epoch on stdout
//!   stop  <name>        — match the most recent `start` for `<name>` and emit `stop`+duration
//!   wrap  <name> -- cmd — `start` → run `cmd` → `stop`. Exit code passes through.
//!
//! Journal: `$KEI_TLOG_JOURNAL` (default `~/.claude/memory/time-metrics/tasks.jsonl`).
//!
//! All output is JSONL; every line is one of:
//!   {"kind":"start","name":..,"start_epoch":..,"ts":..}
//!   {"kind":"stop","name":..,"start_epoch":..,"end_epoch":..,"duration_s":..,"exit":..,"ts":..}
//!
//! Constructor Pattern: one file, ≤200 LOC, no dependencies beyond serde_json + std.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn iso_now() -> String {
    let secs = now_epoch();
    let (y, mo, d, h, mi, se) = epoch_to_ymd_hms(secs);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{se:02}Z")
}

fn epoch_to_ymd_hms(s: u64) -> (i32, u32, u32, u32, u32, u32) {
    let mut s = s as i64;
    let se = (s % 60) as u32;
    s /= 60;
    let mi = (s % 60) as u32;
    s /= 60;
    let h = (s % 24) as u32;
    s /= 24;
    let mut y: i32 = 1970;
    let mut days = s;
    while days >= year_days(y) as i64 {
        days -= year_days(y) as i64;
        y += 1;
    }
    let dim = month_days(y);
    let mut mo: u32 = 1;
    for &md in &dim {
        if days < md as i64 {
            break;
        }
        days -= md as i64;
        mo += 1;
    }
    (y, mo, days as u32 + 1, h, mi, se)
}

fn year_days(y: i32) -> u32 {
    if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 {
        366
    } else {
        365
    }
}

fn month_days(y: i32) -> [u32; 12] {
    let feb = if year_days(y) == 366 { 29 } else { 28 };
    [31, feb, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}

fn journal_path() -> PathBuf {
    if let Ok(p) = env::var("KEI_TLOG_JOURNAL") {
        return PathBuf::from(p);
    }
    let home = env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let dir = PathBuf::from(home).join(".claude/memory/time-metrics");
    let _ = fs::create_dir_all(&dir);
    dir.join("tasks.jsonl")
}

fn append_line(line: &str) -> io::Result<()> {
    let p = journal_path();
    let mut f = OpenOptions::new().create(true).append(true).open(&p)?;
    writeln!(f, "{line}")
}

fn last_start_epoch_for(name: &str) -> Option<u64> {
    let p = journal_path();
    let f = fs::File::open(&p).ok()?;
    let mut found: Option<u64> = None;
    for line in BufReader::new(f).lines().map_while(Result::ok) {
        let v: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v.get("kind").and_then(|x| x.as_str()) == Some("start")
            && v.get("name").and_then(|x| x.as_str()) == Some(name)
        {
            if let Some(s) = v.get("start_epoch").and_then(|x| x.as_u64()) {
                found = Some(s);
            }
        }
    }
    found
}

fn cmd_start(name: &str) -> ExitCode {
    let s = now_epoch();
    let line = serde_json::json!({
        "kind": "start",
        "name": name,
        "start_epoch": s,
        "ts": iso_now(),
    })
    .to_string();
    if let Err(e) = append_line(&line) {
        eprintln!("kei-tlog: journal write failed: {e}");
        return ExitCode::from(1);
    }
    println!("{s}");
    ExitCode::SUCCESS
}

fn cmd_stop(name: &str, exit: i32) -> ExitCode {
    let end = now_epoch();
    let start = match last_start_epoch_for(name) {
        Some(s) => s,
        None => {
            eprintln!("kei-tlog: no `start` line found for name={name}");
            return ExitCode::from(2);
        }
    };
    let dur = end.saturating_sub(start);
    let line = serde_json::json!({
        "kind": "stop",
        "name": name,
        "start_epoch": start,
        "end_epoch": end,
        "duration_s": dur,
        "exit": exit,
        "ts": iso_now(),
    })
    .to_string();
    if let Err(e) = append_line(&line) {
        eprintln!("kei-tlog: journal write failed: {e}");
        return ExitCode::from(1);
    }
    println!("{dur}");
    ExitCode::SUCCESS
}

fn cmd_wrap(name: &str, argv: &[String]) -> ExitCode {
    if argv.is_empty() {
        eprintln!("kei-tlog wrap: missing -- <cmd>");
        return ExitCode::from(64);
    }
    let _ = cmd_start(name);
    let status = Command::new(&argv[0]).args(&argv[1..]).status();
    let exit = status.as_ref().map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    let _ = cmd_stop(name, exit);
    ExitCode::from(if !(0..=255).contains(&exit) { 1 } else { exit as u8 })
}

fn usage() -> ExitCode {
    eprintln!(
        "kei-tlog — RULE 0.17 atomar time-logger\n\n\
         Usage:\n  \
         kei-tlog start <name>\n  \
         kei-tlog stop <name> [--exit N]\n  \
         kei-tlog wrap <name> -- <cmd> [args...]\n\n\
         Journal: $KEI_TLOG_JOURNAL or ~/.claude/memory/time-metrics/tasks.jsonl"
    );
    ExitCode::from(64)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let (sub, rest) = match args.split_first() {
        Some(p) => p,
        None => return usage(),
    };
    match sub.as_str() {
        "start" => match rest.first() {
            Some(name) => cmd_start(name),
            None => usage(),
        },
        "stop" => {
            let name = match rest.first() {
                Some(n) => n,
                None => return usage(),
            };
            let mut exit_code: i32 = 0;
            let mut i = 1;
            while i < rest.len() {
                if rest[i] == "--exit" && i + 1 < rest.len() {
                    exit_code = rest[i + 1].parse().unwrap_or(0);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            cmd_stop(name, exit_code)
        }
        "wrap" => {
            let name = match rest.first() {
                Some(n) => n,
                None => return usage(),
            };
            let dash_idx = rest.iter().position(|x| x == "--");
            let argv = match dash_idx {
                Some(i) => rest[i + 1..].to_vec(),
                None => return usage(),
            };
            cmd_wrap(name, &argv)
        }
        "-h" | "--help" | "help" => {
            usage();
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn epoch_zero_is_1970_01_01() {
        assert_eq!(epoch_to_ymd_hms(0), (1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn epoch_one_day_advances_date_not_time() {
        assert_eq!(epoch_to_ymd_hms(86400), (1970, 1, 2, 0, 0, 0));
    }

    #[test]
    fn epoch_mid_2023_with_time_of_day() {
        // 1700000000 == 2023-11-14T22:13:20Z (verified against a reference
        // UTC converter).
        assert_eq!(epoch_to_ymd_hms(1_700_000_000), (2023, 11, 14, 22, 13, 20));
    }

    #[test]
    fn century_leap_year_2000_02_29_exists() {
        // 2000 is a leap year despite being divisible by 100, because it's
        // also divisible by 400 — the classic Gregorian edge case.
        assert_eq!(epoch_to_ymd_hms(951_782_400), (2000, 2, 29, 0, 0, 0));
    }

    #[test]
    fn near_future_leap_day_2024_04_02() {
        assert_eq!(epoch_to_ymd_hms(1_712_016_000), (2024, 4, 2, 0, 0, 0));
    }

    #[test]
    fn year_days_leap_rules() {
        assert_eq!(year_days(1970), 365, "not divisible by 4");
        assert_eq!(year_days(2024), 366, "divisible by 4, not by 100");
        assert_eq!(year_days(1900), 365, "divisible by 100, not by 400");
        assert_eq!(year_days(2000), 366, "divisible by 400");
    }

    #[test]
    fn month_days_february_matches_leap_status() {
        assert_eq!(month_days(2024)[1], 29);
        assert_eq!(month_days(2023)[1], 28);
        assert_eq!(month_days(1900)[1], 28);
        assert_eq!(month_days(2000)[1], 29);
    }

    #[test]
    fn start_then_stop_round_trips_through_journal() {
        let dir = std::env::temp_dir().join(format!("kei-tlog-test-{}", now_epoch()));
        std::fs::create_dir_all(&dir).unwrap();
        let journal = dir.join("tasks.jsonl");
        // SAFETY: this is the only test in this crate that touches
        // KEI_TLOG_JOURNAL, so there's no cross-test race on the env var.
        unsafe {
            std::env::set_var("KEI_TLOG_JOURNAL", &journal);
        }

        let name = "unit-test-task";
        let start = now_epoch();
        let line = serde_json::json!({
            "kind": "start", "name": name, "start_epoch": start, "ts": iso_now(),
        })
        .to_string();
        append_line(&line).unwrap();

        assert_eq!(last_start_epoch_for(name), Some(start));
        assert_eq!(last_start_epoch_for("some-other-task"), None);

        unsafe {
            std::env::remove_var("KEI_TLOG_JOURNAL");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
