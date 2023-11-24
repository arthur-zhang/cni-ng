use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use log::LevelFilter;
use simplelog::{
    ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode, WriteLogger,
};

use crate::prelude::*;

// pub const LOG_DIR: &'static str = "/var/log/cni/";
pub const LOG_DIR: &str = "/tmp/log/cni/";

pub fn init(log_name: impl AsRef<Path>) -> CniResult<()> {
    let config = {
        let mut builder = ConfigBuilder::new();
        builder.set_thread_level(LevelFilter::Info);
        builder.set_target_level(LevelFilter::Info);
        builder.build()
    };
    let term_logger = TermLogger::new(
        LevelFilter::Info,
        config.clone(),
        TerminalMode::Stderr,
        ColorChoice::Never,
    );
    let log_dir = PathBuf::from_str(LOG_DIR)?;
    let log_file = log_dir.join(log_name);
    if !log_dir.exists() {
        std::fs::create_dir_all(log_dir)?;
    }
    let file_logger = WriteLogger::new(
        LevelFilter::Debug,
        config.clone(),
        OpenOptions::new()
            .append(true)
            .create(true)
            .open(log_file)?,
    );
    CombinedLogger::init(vec![
        // term_logger,
        file_logger,
    ])?;
    Ok(())
}
