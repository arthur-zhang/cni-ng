#[macro_use]
extern crate log;
extern crate simplelog;

use std::io::{stdin, stdout};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

use cni_core::prelude::*;
use cni_core::skel::CmdArgs;
use cni_core::types::{IPAMConfig, SuccessReply};
use cni_core::{logger, skel};

use crate::types::NetConf;

mod types;

// static IPAM is very simple IPAM plugin that assigns IPv4 and IPv6 addresses statically to
// container. This will be useful in debugging purpose and in case of assign same IP address
// in different vlan/vxlan to containers.
//
// Example configuration
// {
// 	"ipam": {
// 		"type": "static",
// 		"addresses": [
// 			{
// 				"address": "10.10.0.1/24",
// 				"gateway": "10.10.0.254"
// 			},
// 			{
// 				"address": "3ffe:ffff:0:01ff::1/64",
// 				"gateway": "3ffe:ffff:0::1"
// 			}
// 		],
// 		"routes": [
// 			{ "dst": "0.0.0.0/0" },
// 			{ "dst": "192.168.0.0/16", "gw": "10.10.5.1" },
// 			{ "dst": "3ffe:ffff:0:01ff::1/64" }
// 		],
// 		"dns": {
// 			"nameservers" : ["8.8.8.8"],
// 			"domain": "example.com",
// 			"search": [ "example.com" ]
// 		}
// 	}
// }

fn main() -> CniResult<()> {
    logger::init("ipam_static.log")?;
    skel::plugin_main(
        |args| cmd_add(args),
        |args| cmd_add(args),
        |args| cmd_add(args),
    )?;
    Ok(())
}

fn cmd_add(cmd_args: CmdArgs) -> CniResult<()> {
    info!("cmd_args: {:?}", cmd_args);
    let mut ipam = load_ipam_conf(&cmd_args.args)?;
    let ipam_type = ipam.plugin;
    if ipam_type != "static" {
        panic!("only support static ipam");
    }
    let result = SuccessReply {
        cni_version: "1.0.0".to_string(),
        interfaces: vec![],
        ips: ipam.addresses.take().unwrap_or_default(),
        routes: ipam.routes.take().unwrap_or_default(),
        dns: ipam.dns.take().unwrap_or_default(),
        specific: Default::default(),
    };

    serde_json::to_writer(stdout(), &result).expect("writing to stdout should not fail");
    Ok(())
}

fn cmd_del(cmd_args: CmdArgs) -> CniResult<()> {
    todo!()
}

fn cmd_check(cmd_args: CmdArgs) -> CniResult<()> {
    todo!()
}

fn load_ipam_conf(env_args: &str) -> CniResult<IPAMConfig> {
    let net_config: NetConf = serde_json::from_reader(stdin()).unwrap();
    let ipam = net_config
        .ipam
        .ok_or(anyhow!("IPAM config missing 'ipam' key"))?;
    Ok(ipam)
}
