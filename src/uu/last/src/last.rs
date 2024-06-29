use uucore::{format_usage, help_about, help_usage};
use clap::{crate_version, Arg, ArgAction, Command};

mod platform;

mod options {
    pub const SYSTEM: &str = "system";
    pub const FILE: &str = "file";
}

const ABOUT: &str = help_about!("last.md");
const USAGE: &str = help_usage!("last.md");

#[uucore::main]
use platform::uumain;

pub fn uu_app() -> Command {
    Command::new(uucore::util_name())
        .version(crate_version!())
        .about(ABOUT)
        .override_usage(format_usage(USAGE))
        .infer_long_args(true)
        .arg(
            Arg::new(options::FILE)
                .short('f')
                .long("file")
                .action(ArgAction::Set)
                .default_value("/var/log/wtmp")
                .help("use a specific file instead of /var/log/wtmp")
                .required(false)
        )
        .arg(
            Arg::new(options::SYSTEM)
                .short('x')
                .long(options::SYSTEM)
                .action(ArgAction::SetTrue)
                .required(false)
                .help("display system shutdown entries and run level changes")
        )
}