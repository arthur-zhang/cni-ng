use anyhow::anyhow;
use netlink_ng::Addr;

use cni_core::types::ExecResult;

pub fn config_interface(if_name: &str, exec_result: &ExecResult) -> anyhow::Result<()> {
    let link = netlink_ng::link_by_name(if_name)?;
    let link = link.ok_or(anyhow!("link not found"))?;

    let ips = exec_result.ips.as_ref().ok_or(anyhow!("ips not found"))?;

    for ip in ips {
        if ip.interface.is_none() {
            continue;
        }
        // add address to veth interface
        netlink_ng::addr_add(
            &link,
            &Addr {
                ipnet: ip.address.clone(),
                ..Default::default()
            },
        )?;
    }

    netlink_ng::link_set_up(&link)?;

    if let Some(routes) = &exec_result.routes {
        for route in routes {
            let route = netlink_ng::types::Route {
                dst: Some(route.dst.clone()),
                link_index: link.attrs().index,
                gw: route.gw.clone(),
                ..Default::default()
            };
            netlink_ng::route_add_ecmp(&route)?;
        }
    }

    Ok(())
}
