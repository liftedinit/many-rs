use std::os::unix::ffi::OsStrExt;
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::Subscriber;

pub mod error;

#[derive(clap::ArgEnum, Clone, Debug)]
enum LogStrategy {
    Terminal,
    Syslog,
}

#[derive(clap::Args, Debug, Clone)]
pub struct Verbosity {
    /// Increase output logging verbosity to DEBUG level.
    #[clap(
        long,
        short = 'v',
        action = clap::ArgAction::Count,
        global = true,
    )]
    verbose: u8,

    /// Suppress all output logging. Can be used multiple times to suppress more.
    #[clap(
        long,
        short = 'q',
        action = clap::ArgAction::Count,
        global = true,
    )]
    quiet: u8,
}

impl Verbosity {
    pub fn level(&self) -> LevelFilter {
        let verbose_level = 2 + (self.verbose as i8) - (self.quiet as i8);
        match verbose_level {
            i8::MIN..=-1 => LevelFilter::OFF,
            0 => LevelFilter::ERROR,
            1 => LevelFilter::WARN,
            2 => LevelFilter::INFO,
            3 => LevelFilter::DEBUG,
            4..=i8::MAX => LevelFilter::TRACE,
        }
    }
}

#[derive(clap::Args, Debug, Clone)]
pub struct CommonCliFlags {
    #[clap(flatten)]
    verbosity: Verbosity,

    /// Use given logging strategy
    #[clap(long, arg_enum, default_value_t = LogStrategy::Terminal)]
    logmode: LogStrategy,
}

impl CommonCliFlags {
    pub fn init_logging(&self) -> Result<(), String> {
        let subscriber = Subscriber::builder().with_max_level(self.verbosity.level());

        match self.logmode {
            LogStrategy::Terminal => {
                let subscriber = subscriber.with_writer(std::io::stderr);
                subscriber.init();
            }
            LogStrategy::Syslog => {
                let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
                let process_name = exe_path.file_name().unwrap();
                let identity =
                    std::ffi::CString::new(process_name.as_bytes()).map_err(|e| e.to_string())?;
                let (options, facility) = Default::default();
                let syslog = syslog_tracing::Syslog::new(identity, options, facility)
                    .ok_or_else(|| "Could not create syslog logger.".to_string())?;

                let subscriber = subscriber.with_ansi(false).with_writer(syslog);
                subscriber.init();
                log_panics::init();
            }
        };

        Ok(())
    }
}
