#[macro_use]
extern crate log;
extern crate simplelog;

use std::collections::HashMap;
use std::fs::File;
use std::io::stdout;

use anyhow::bail;
use ipnetwork::{Ipv4Network, Ipv6Network};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use cni_core::skel::CmdArgs;
use cni_core::{logger, skel};

const DEFAULT_SUBNET_FILE: &str = "/run/flannel/subnet.env";
const DEFAULT_DATA_DIR: &str = "/var/lib/cni/flannel";

fn main() -> anyhow::Result<()> {
    logger::init("flannel-plugin.log")?;
    skel::plugin_main(
        |args| cmd_add(args),
        |args| cmd_add(args),
        |args| cmd_add(args),
    )?;
    Ok(())
}

fn cmd_add(cmd_args: CmdArgs) -> anyhow::Result<()> {
    println!("cmd add ...............");
    let mut net_conf = load_flannel_net_conf()?;
    let subnet_env = load_flannel_subnet_env(net_conf.subnet_file.as_ref().unwrap())?;
    println!("subnet_env: {:?}", subnet_env);

    match &net_conf.delegate {
        None => {
            net_conf.delegate = Some(HashMap::new());
        }
        Some(delegate) => {
            if !delegate
                .get("type")
                .map(|it| it.is_string())
                .unwrap_or(false)
            {
                bail!("'delegate' dictionary, if present, must have (string) 'type' field");
            }
            if delegate.get("name").is_none() {
                bail!("'delegate' dictionary must not have 'name' field, it'll be set by flannel");
            }
            if delegate.get("ipam").is_none() {
                bail!("'delegate' dictionary must not have 'ipam' field, it'll be set by flannel");
            }
        }
    }

    let delegate_mut = net_conf.delegate.as_mut().unwrap();
    delegate_mut.insert("name".into(), Value::String(net_conf.name.clone()));
    delegate_mut.entry("type".into()).or_insert("bridge".into());

    if !delegate_mut.contains_key("ipMasq") {
        delegate_mut.insert("ipMasq".into(), Value::Bool(!subnet_env.ipmasq.unwrap()));
    }

    delegate_mut
        .entry("mtu".into())
        .or_insert(Value::Number(subnet_env.mtu.unwrap().into()));

    if delegate_mut.get("type").unwrap().as_str() == Some("bridge") {
        delegate_mut
            .entry("isGateway".into())
            .or_insert(Value::Bool(true));
    }
    if !net_conf.cni_version.is_empty() {
        delegate_mut.insert(
            "cniVersion".into(),
            Value::String(net_conf.cni_version.clone()),
        );
    }
    get_delegate_ipam(&mut net_conf, &subnet_env)?;
    let delegate_mut = net_conf.delegate.as_mut().unwrap();
    delegate_mut.insert("ipam".into(), Value::Object(net_conf.ipam.clone().unwrap()));

    println!(
        "delegate_conf: {}",
        serde_json::to_string_pretty(&net_conf.delegate)?
    );

    delegate_add(
        &cmd_args.container_id,
        net_conf.data_dir.as_ref().unwrap(),
        net_conf.delegate.as_ref().unwrap(),
    )?;
    Ok(())
}

fn delegate_add(
    _cid: &str,
    _data_dir: &str,
    delegate_conf: &HashMap<String, Value>,
) -> anyhow::Result<()> {
    let net_conf_bytes = serde_json::to_string(&delegate_conf)?;
    println!("net_conf_bytes: {}", net_conf_bytes);

    let plugin_type = delegate_conf.get("type").unwrap().as_str().unwrap();
    let result = invoke::delegate_add(plugin_type, net_conf_bytes.as_bytes())?;
    serde_json::to_writer(stdout(), &result).expect("writing to stdout should not fail");
    Ok(())
}

fn get_delegate_ipam(n: &mut NetConf, subnet_env: &SubnetEnv) -> anyhow::Result<()> {
    if n.ipam.is_none() {
        n.ipam = Some(Map::new());
    }

    let ipam = n.ipam.as_mut().unwrap();
    ipam.entry("type".to_string())
        .or_insert("host-local".into());

    let mut ranges = vec![];
    if let Some(sn) = subnet_env.sn {
        ranges.push(Value::Array(vec![json!({"subnet": sn.to_string()})]))
    }

    ipam.insert("ranges".into(), Value::Array(ranges));

    let routes = subnet_env
        .nws
        .iter()
        .map(|it| json!({"dst": it.to_string()}))
        .collect::<Vec<_>>();
    ipam.insert("routes".into(), Value::Array(routes));
    println!("{}", serde_json::to_string(&ipam)?);

    Ok(())
}

fn load_flannel_net_conf() -> anyhow::Result<NetConf> {
    let f = File::open("/home/arthur/cni-rs/flannel.stdin.json")?;
    // let mut n: NetConf = serde_json::from_reader(stdin())?;
    let mut n: NetConf = serde_json::from_reader(&f)?;
    n.subnet_file.get_or_insert(DEFAULT_SUBNET_FILE.into());
    n.data_dir.get_or_insert(DEFAULT_DATA_DIR.into());
    Ok(n)
}

fn load_flannel_subnet_env(path: &str) -> anyhow::Result<SubnetEnv> {
    let content = std::fs::read_to_string(path)?;
    let mut subnet_env = SubnetEnv {
        nws: Vec::new(),
        sn: None,
        ip6_nws: Vec::new(),
        ip6_sn: None,
        mtu: None,
        ipmasq: None,
    };
    for line in content.lines() {
        for (k, v) in line.split_once('=').into_iter() {
            match k {
                "FLANNEL_NETWORK" => {
                    subnet_env.nws = v
                        .split(',')
                        .map(|it| it.parse::<Ipv4Network>())
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "FLANNEL_IPV6_NETWORK" => {
                    subnet_env.ip6_nws = v
                        .split(',')
                        .map(|it| it.parse::<Ipv6Network>())
                        .collect::<Result<Vec<_>, _>>()?;
                }
                "FLANNEL_SUBNET" => {
                    let sn = v.parse::<Ipv4Network>()?;
                    subnet_env.sn = Some(Ipv4Network::with_netmask(sn.network(), sn.mask())?);
                }
                "FLANNEL_IPV6_SUBNET" => {
                    let sn = v.parse::<Ipv6Network>()?;
                    subnet_env.ip6_sn = Some(Ipv6Network::with_netmask(sn.network(), sn.mask())?);
                }
                "FLANNEL_MTU" => {
                    subnet_env.mtu = Some(v.parse::<u32>()?);
                }
                "FLANNEL_IPMASQ" => {
                    subnet_env.ipmasq = Some(v.parse::<bool>()?);
                }
                _ => {}
            }
        }
    }
    Ok(subnet_env)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetConf {
    pub cni_version: String,
    pub name: String,
    #[serde(rename = "type")]
    pub plugin: String,
    #[serde(rename = "subnetFile")]
    pub subnet_file: Option<String>,
    #[serde(rename = "dataDir")]
    pub data_dir: Option<String>,
    #[serde(rename = "delegate")]
    pub delegate: Option<HashMap<String, Value>>,
    #[serde(rename = "ipam")]
    pub ipam: Option<Ipam>,
    #[serde(rename = "runtimeConfig", skip_serializing_if = "Option::is_none")]
    pub runtime_config: Option<HashMap<String, serde_json::Value>>,
}

pub type Ipam = Map<String, Value>;

#[derive(Debug)]
pub struct SubnetEnv {
    nws: Vec<Ipv4Network>,
    sn: Option<Ipv4Network>,
    ip6_nws: Vec<Ipv6Network>,
    ip6_sn: Option<Ipv6Network>,
    mtu: Option<u32>,
    ipmasq: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1() {
        let ip_str = "172.17.78.1/24";
        let ip: Ipv4Network = ip_str.parse().unwrap();

        let ip_22 = Ipv4Network::with_netmask(ip.network(), ip.mask()).unwrap();
        println!(">>>>>>>>>>>{}", ip_22);

        println!("{}", ip.network());
        println!("{}", ip.ip());
        println!("{}", ip.mask());
        println!("{}", ip.prefix());

        println!("{}", ip);
    }
}
