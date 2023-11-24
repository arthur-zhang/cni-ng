use std::fmt;
use std::str::FromStr;

use anyhow::anyhow;

use crate::prelude::CniResult;

#[derive(Debug)]
pub struct CmdArgs {
    pub container_id: String,
    pub netns: String,
    pub if_name: String,
    pub args: String,
    pub path: String,
}

pub enum Cmd {
    Add,
    Del,
    Check,
    Version,
}

impl fmt::Display for Cmd {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Cmd::Add => write!(f, "ADD"),
            Cmd::Del => write!(f, "DEL"),
            Cmd::Check => write!(f, "CHECK"),
            Cmd::Version => write!(f, "VERSION"),
        }
    }
}

impl FromStr for Cmd {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ADD" => Ok(Cmd::Add),
            "DEL" => Ok(Cmd::Del),
            "CHECK" => Ok(Cmd::Check),
            "VERSION" => Ok(Cmd::Version),
            _ => Err(anyhow!("unknown command: {}", s)),
        }
    }
}

pub type CmdOutput = CniResult<()>;
pub type CmdFn = fn(CmdArgs) -> CmdOutput;
pub type PluginResult = CniResult<()>;

pub fn plugin_main(add_fn: CmdFn, del_fn: CmdFn, check_fn: CmdFn) -> PluginResult {
    let (cmd, args) = get_cmd_args_from_env()?;
    match cmd {
        Cmd::Add => {
            add_fn(args)?;
        }
        Cmd::Del => {
            del_fn(args)?;
        }
        Cmd::Check => {
            check_fn(args)?;
        }
        Cmd::Version => {
            todo!()
        }
    }

    Ok(())
}

pub fn get_cmd_args_from_env() -> CniResult<(Cmd, CmdArgs)> {
    let cmd = std::env::var("CNI_COMMAND")
        .unwrap_or("".into())
        .as_str()
        .parse()?;
    let container_id = std::env::var("CNI_CONTAINERID").unwrap_or("".into());
    let netns = std::env::var("CNI_NETNS").unwrap_or("".into());
    let if_name = std::env::var("CNI_IFNAME").unwrap_or("".into());
    let args = std::env::var("CNI_ARGS").unwrap_or("".into());

    // List of paths to search for CNI plugin executables.
    // Paths are separated by an OS-specific list separator; for example ‘:’ on Linux and ‘;’ on Windows
    let path = std::env::var("CNI_PATH").unwrap_or("".into());
    Ok((
        cmd,
        CmdArgs {
            container_id,
            netns,
            if_name,
            args,
            path,
        },
    ))
}
