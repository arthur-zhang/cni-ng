use std::io::{stdin, stdout};
use std::sync::Arc;

use anyhow::bail;

use cni_core::skel;
use cni_core::skel::CmdArgs;
use cni_core::types::ExecResult;

use crate::allocator::IpAllocator;
use crate::config::{IPAMConfig, Net};
use crate::disk::Store;
use crate::range_set::RangeSetExt;

mod allocator;
mod config;
mod disk;
mod range;
mod range_set;

// host-local IPAM allocates IPv4 and IPv6 addresses out of a specified address range.
// Optionally, it can include a DNS configuration from a resolv.conf file on the host.
fn main() -> anyhow::Result<()> {
    // logger::init("ipam_host_local.log")?;
    skel::plugin_main(
        |args| cmd_add(args),
        |args| cmd_add(args),
        |args| cmd_add(args),
    )?;
    Ok(())
}

fn load_ipam_config() -> anyhow::Result<(IPAMConfig, String)> {
    let mut n: Net = serde_json::from_reader(stdin())?;
    // todo add resolv.conf
    if n.ipam.ranges.is_empty() {
        bail!("no IP ranges specified")
    }

    for entry in n.ipam.ranges.iter_mut() {
        entry.canonicalize()?;
    }

    let l = n.ipam.ranges.len();
    for i in 0..l {
        for j in i + 1..l {
            if n.ipam.ranges[i].overlap(&n.ipam.ranges[j]) {
                bail!("range set {} overlaps with {}", i, i + j + 1)
            }
        }
    }
    n.ipam.name = Some(n.name.clone());
    Ok((n.ipam, n.cni_version.clone()))
}

fn cmd_add(cmd_args: CmdArgs) -> anyhow::Result<()> {
    let (ipam_config, cni_version) = load_ipam_config()?;
    let store = Arc::new(Store::new(ipam_config.data_dir)?);

    // let requested_ips: HashMap<String, IpAddr> = HashMap::new();

    let mut allocators: Vec<IpAllocator> = vec![];
    let mut exec_result = ExecResult::default();

    let mut ips = vec![];
    for (idx, rangeset) in ipam_config.ranges.into_iter().enumerate() {
        // println!("x: {:?}", x);
        let allocator = IpAllocator::new(rangeset, store.clone(), idx);
        let result = allocator.get(&cmd_args.container_id, &cmd_args.if_name, None);
        match result {
            Ok(ip) => {
                ips.push(ip);
            }
            Err(e) => {
                for alloc in &allocators {
                    let _ = alloc.release(&cmd_args.container_id, &cmd_args.if_name);
                }
                return Err(e);
            }
        }
        allocators.push(allocator);
    }

    exec_result.cni_version = Some(cni_version);
    exec_result.ips = Some(ips);
    exec_result.routes = ipam_config.routes;
    serde_json::to_writer(stdout(), &exec_result).expect("writing to stdout should not fail");
    Ok(())
}
