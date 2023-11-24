use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use log::info;

use cni_core::types::ExecResult;

pub trait Args {
    fn as_env(&self) -> HashMap<String, String>;
}

pub struct CNIArgs {
    pub command: String,
    pub containerid: String,
    pub netns: String,
    pub args: HashMap<String, String>,
    pub ifname: String,
    pub path: String,
}

fn stringify_args(args: &HashMap<String, String>) -> String {
    let mut result = String::new();
    for (key, value) in args {
        result.push_str(&format!("{}={};", key, value));
    }
    result
}

impl Args for CNIArgs {
    fn as_env(&self) -> HashMap<String, String> {
        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.insert("CNI_COMMAND".to_string(), self.command.clone());
        env.insert("CNI_CONTAINERID".to_string(), self.containerid.clone());
        env.insert("CNI_NETNS".to_string(), self.netns.clone());
        env.insert("CNI_ARGS".to_string(), stringify_args(&self.args));
        env.insert("CNI_IFNAME".to_string(), self.ifname.clone());
        env.insert("CNI_PATH".to_string(), self.path.clone());
        env
    }
}

pub struct DelegateArgs {
    pub command: String,
}

impl Args for DelegateArgs {
    fn as_env(&self) -> HashMap<String, String> {
        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.insert("CNI_COMMAND".to_string(), self.command.clone());
        env
    }
}

pub fn delegate_add(plugin: &str, net_conf: &[u8]) -> anyhow::Result<ExecResult> {
    let plugin_path = delegate_common(plugin)?;
    info!("plugin_path: {:?}", plugin_path);
    let res = exec_plugin_with_result(
        &plugin_path,
        net_conf,
        DelegateArgs {
            command: "ADD".to_string(),
        },
    )?;
    let result: ExecResult = serde_json::from_slice(&res)?;

    Ok(result)
}

pub fn delegate_common(plugin: &str) -> anyhow::Result<PathBuf> {
    let cni_path = std::env::var("CNI_PATH").unwrap_or("".into());
    info!("cni_path: {:?}", cni_path);
    let paths = cni_path.split(':').map(Path::new).collect::<Vec<_>>();

    let plugin_exec_path = find_exec_in_path(plugin, paths).ok_or(anyhow::anyhow!(
        "plugin {} not found in CNI_PATH: {}",
        plugin,
        cni_path
    ))?;

    Ok(plugin_exec_path)
}

fn exec_plugin_with_result(
    plugin_path: &Path,
    stdin_data: &[u8],
    args: impl Args,
) -> anyhow::Result<Vec<u8>> {
    println!("plugin_path: {:?}", plugin_path);
    println!("env: {:?}", args.as_env());
    println!("stdin_data: {}", std::str::from_utf8(stdin_data).unwrap());

    let mut child = Command::new(plugin_path.as_os_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .envs(args.as_env())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(stdin_data)?;
    drop(stdin);
    let mut stdout = child.stdout.take().unwrap();
    let mut buffer = Vec::new();
    stdout.read_to_end(&mut buffer)?;
    let exit_status = child.wait()?;

    if let Some(code) = exit_status.code() {
        if code != 0 {
            println!("{}", exit_status.to_string());
            println!("{}", std::str::from_utf8(&buffer).unwrap());
            return Err(anyhow::anyhow!(
                "plugin exited with non-zero exit code: {}",
                code
            ));
        }
    }
    Ok(buffer)
}

fn find_exec_in_path(plugin: &str, paths: Vec<&Path>) -> Option<PathBuf> {
    for path in paths {
        let full_path = path.join(plugin);
        if full_path.exists() && full_path.is_file() {
            return Some(full_path);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use log::info;

    use crate::delegate_add;

    #[test]
    fn test_run_plugin() {
        let net_conf = r#"
        {
  "cniVersion": "1.0.0",
  "name": "mynet",
  "type": "bridge",
  "bridge": "br666",
  "isDefaultGateway": true,
  "forceAddress": false,
  "ipMasq": true,
  "hairpinMode": true,
  "ipam": {
    "type": "static",
    "addresses": [
      {
        "address": "10.10.0.1/24",
        "gateway": "10.10.0.254"
      },
      {
        "address": "3ffe:ffff:0:01ff::1/64",
        "gateway": "3ffe:ffff:0::1"
      }
    ]
  }
}
       "#;
        let a = delegate_add("static", net_conf.as_bytes()).unwrap();
        info!("a: {:?}", a);
    }
}
