// This file is part of the uutils util-linux package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

// Specific implementation for OpenBSD: tool unsupported (utmpx not supported)

use crate::uu_app;
use crate::options;

use uucore::error::UResult;

use uucore::utmpx::time::OffsetDateTime;
use uucore::utmpx::{time, Utmpx};

use std::fmt::Write;

use std::path::PathBuf;

fn get_long_usage() -> String {
    format!(
        "For more details see last(1)."
    )
}

const WTMP_PATH: &str = "/var/log/wtmp";

pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = uu_app()
        .after_help(get_long_usage())
        .try_get_matches_from(args)?;

    let system = matches.get_flag(options::SYSTEM);

    let time_format = "short".to_string();  // TODO implement time formatting later;

    
    let file: String = if let Some(files) = matches.get_one::<String>(options::FILE) {
        files.to_string()
    } else {
        WTMP_PATH.to_string()
    };

    let users: Option<Vec<String>> = None; // TODO implement user searching

    let mut last = Last {
        last_reboot_ut: None,
        last_shutdown_ut: None,
        last_dead_ut: vec![],
        system,
        file: file.to_string(),
        users,
        time_format
    };

    last.exec()
}

const RUN_LEVEL_STR: &str = "runlevel";
const REBOOT_STR: &str = "reboot";
const SHUTDOWN_STR: &str = "shutdown";

struct Last {
    last_reboot_ut: Option<Utmpx>,
    last_shutdown_ut: Option<Utmpx>,
    last_dead_ut: Vec<Utmpx>,
    system: bool,
    file: String,
    time_format: String,
    users: Option<Vec<String>>,
}

#[inline]
fn calculate_time_delta(curr_datetime: &OffsetDateTime, last_datetime: &OffsetDateTime) -> time::Duration {
    let curr_duration = time::Duration::new(
       curr_datetime.unix_timestamp(),
       curr_datetime.nanosecond().try_into().unwrap() // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    let last_duration = time::Duration::new(
        last_datetime.unix_timestamp(),
        last_datetime.nanosecond().try_into().unwrap() // nanosecond value is always a value between 0 and 1.000.000.000, shouldn't panic
    );

    last_duration - curr_duration
}

#[inline]
fn duration_string(duration: time::Duration) -> String {
    let mut seconds = duration.whole_seconds();
    
    let days = seconds / 86400;
    seconds = seconds - (days * 86400);
    let hours = seconds / 3600;
    seconds = seconds - (hours * 3600);
    let minutes =  seconds / 60;

    if days > 0 {
        format!("({}+{:0>2}:{:0>2})", days, hours, minutes)
    } else {
        format!("({:0>2}:{:0>2})", hours, minutes)
    }
}

impl Last {
    #[allow(clippy::cognitive_complexity)]
    fn exec(&mut self) -> UResult<()> {
        let mut ut_stack: Vec<Utmpx> = vec![];
        Utmpx::iter_all_records_from(&self.file).for_each(|ut| {
            ut_stack.push(ut) // For 'last' output, older output needs to be printed last (FILO), as UtmpxIter does not implement Rev trait
                              // A better implementation might include implementing UtmpxIter as doubly linked
        });

        while let Some(ut) = ut_stack.pop() {
            // println!("|{}| |{}| |{}|", ut.user(), time_string(&ut), ut.tty_device());
            if ut.is_user_process() {
                let mut dead_proc: Option<Utmpx> = None;
                if let Some(pos) = self.last_dead_ut.iter().position(|dead_ut| { ut.tty_device() == dead_ut.tty_device() }) {
                    dead_proc = Some(self.last_dead_ut.swap_remove(pos));
                }
                self.print_user(&ut, dead_proc.as_ref());
            } else if ut.user() == RUN_LEVEL_STR {
                self.print_runlevel(&ut);
            } else if ut.user() == SHUTDOWN_STR {
                self.print_shutdown(&ut);
                self.last_shutdown_ut = Some(ut);
            } else if ut.user() == REBOOT_STR {
                self.print_reboot(&ut);
                self.last_reboot_ut = Some(ut);
            } else if ut.user() == "" { // Dead process end date
                self.last_dead_ut.push(ut);
            }
        }
        
        Ok(())
    }
    
    #[inline]
    fn time_string(&self, ut: &Utmpx) -> String {
        let description = match self.time_format.as_str() {
            "short" => {"[month repr:short] [day padding:space] [hour]:[minute]"}
            _ => {return "".to_string()}
        };

        // "%b %e %H:%M"
        let time_format: Vec<time::format_description::FormatItem> =
            time::format_description::parse(description)
                .unwrap();
        ut.login_time().format(&time_format).unwrap() // LC_ALL=C
    }

    #[inline]
    fn end_time_string(
        &self,
        user_process_str: Option<&str>,
        end_ut: &OffsetDateTime
    ) -> String {
        match user_process_str {
            Some(val) => { val.to_string() }
            _ => {
                let description = match self.time_format.as_str() {
                    "short" => {"[hour]:[minute]"}
                    _ => {return "".to_string()}
                };

                // "%H:%M"
                let time_format: Vec<time::format_description::FormatItem> =
                time::format_description::parse(description)
                    .unwrap();
                end_ut.format(&time_format).unwrap() // LC_ALL=C
            }
        }
    }

    #[inline]
    fn end_state_string(&self, ut: &Utmpx, dead_ut: Option<&Utmpx>) -> (String, String) {
        // This function takes a considerable amount of CPU cycles to complete;
        // root cause seems to be the ut.login_time function, which reads a
        // file to determine local offset for UTC. Perhaps this function
        // should be updated to save that UTC offset for subsequent calls
        let mut proc_status: Option<&str> = None;
        let curr_datetime = ut.login_time();

        if let Some(dead) = dead_ut {
            let dead_datetime = dead.login_time();
            let time_delta = duration_string(calculate_time_delta(&curr_datetime, &dead_datetime));
            return (self.end_time_string(proc_status, &dead_datetime), time_delta.to_string())
        }
        
        let reboot_datetime: Option<OffsetDateTime>;
        let shutdown_datetime: Option<OffsetDateTime>;
        if let Some(reboot) = &self.last_reboot_ut {
            reboot_datetime = Some(reboot.login_time());
        } else {
            reboot_datetime = None;
        }

        if let Some(shutdown) = &self.last_shutdown_ut {
            shutdown_datetime = Some(shutdown.login_time());
        } else {
            shutdown_datetime = None;
        }

        // let last_datetimes_tuple = (reboot_datetime, shutdown_datetime);

        if reboot_datetime.is_none() && shutdown_datetime.is_none() {
            if ut.is_user_process() {
                (" - still logged in".to_string(), "".to_string())
            } else { 
                (" - still running".to_string(), "".to_string()) 
            }
        } else {
            let reboot = reboot_datetime.unwrap_or_else(|| { time::OffsetDateTime::from_unix_timestamp(0).unwrap() });
            let shutdown = shutdown_datetime.unwrap_or_else(|| { time::OffsetDateTime::from_unix_timestamp(0).unwrap() });
            if reboot >= shutdown {
                let time_delta = duration_string(calculate_time_delta(&curr_datetime, &shutdown));
                if ut.is_user_process() { proc_status = Some("down"); }
                (self.end_time_string(proc_status, &shutdown), time_delta.to_string())
            } else {
                let time_delta = duration_string(calculate_time_delta(&curr_datetime, &reboot));
                if ut.is_user_process() { proc_status = Some("crash"); }
                (self.end_time_string(proc_status, &reboot), time_delta.to_string())
            }
        }
    }

    #[inline]
    fn print_runlevel(&self, ut: &Utmpx) -> bool {
        if self.system {
            let curr = (ut.pid() % 256) as u8 as char;
            let runlvline = format!("(to lvl {curr})");
            let (end_date, delta) = self.end_state_string(ut, None);
            let host = ut.host();
            self.print_line(
                RUN_LEVEL_STR,
                &runlvline,
                &self.time_string(ut),
                &host,
                &end_date,
                &delta
            );
            true
        } else {
            false
        }
    }

    #[inline]
    fn print_shutdown(&self, ut: &Utmpx) -> bool {
        if let Some(users) = &self.users {
            if !users.iter().any(|val| {val.as_str().trim() == "system down" || val.as_str().trim() == ut.user().trim()}) {
                return false
            }
        }
        let host = ut.host();
        if self.system {
            let (end_date, delta) = self.end_state_string(ut, None);
            self.print_line(
                SHUTDOWN_STR,
                "system down",
                &self.time_string(ut),
                &host,
                &end_date,
                &delta
            );
            true
        } else {
            false
        }
    }

    #[inline]
    fn print_reboot(&self, ut: &Utmpx) -> bool {
        let (end_date, delta) = self.end_state_string(ut, None);
        let host = ut.host();
        self.print_line(
            REBOOT_STR,
            "system boot",
            &self.time_string(ut),
            &host,
            &end_date,
            &delta
        );

        true
    }

    #[inline]
    fn print_user(&self, ut: &Utmpx, dead_ut: Option<&Utmpx>) -> bool {
        let mut p = PathBuf::from("/dev");
        p.push(ut.tty_device().as_str());
        let host = ut.host();

        let (end_date, delta) = self.end_state_string(ut, dead_ut);

        self.print_line(
            ut.user().as_ref(),
            ut.tty_device().as_ref(),
            self.time_string(ut).as_str(),
            &host,
            &end_date,
            &delta
        );

        true
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn print_line(
        &self,
        user: &str,
        line: &str,
        time: &str,
        host: &str,
        end_time: &str,
        delta: &str
    ) {
        let mut buf = String::with_capacity(64);
        let host_to_print = host.get(0..16).unwrap_or(host);

        write!(buf, "{user:<8}").unwrap();
        write!(buf, " {line:<12}").unwrap();
        write!(buf, " {host_to_print:<16}").unwrap();

        let time_size = 3 + 2 + 2 + 1 + 2;

        write!(buf, " {time:<time_size$}").unwrap();
        write!(buf, " - {end_time:<8}").unwrap();

        write!(buf, " {delta:^6}").unwrap();
        println!("{}", buf.trim_end());
    }
}
