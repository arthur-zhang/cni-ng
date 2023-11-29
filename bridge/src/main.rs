use std::io::{Read, stdin, stdout, Write};
use std::net::Ipv4Addr;

use anyhow::{anyhow, bail};
use ipnetwork::{IpNetwork, Ipv4Network};
use log::info;
use netlink_ng::{Link, LinkAttrs, LinkKind, TryAsLinkIndex};
use netlink_ng::nl_type::{Bridge, Family, FAMILY_V4, FAMILY_V6};
use netns_ng::Netns;
use serde::{Deserialize, Serialize};

use cni_core::{logger, skel};
use cni_core::error::is_already_exists_error;
use cni_core::prelude::CniResult;
use cni_core::skel::CmdArgs;
use cni_core::types::{ExecResult, Interface, MacAddr, Route};

use crate::types::NetConf;

mod types;

fn main() {
    let _ = logger::init("bridge.log");

    let res = skel::plugin_main(
        |args| cmd_add(args),
        |args| cmd_add(args),
        |args| cmd_add(args),
    );
    info!("res: {:?}", res);
}

fn cmd_add(args: CmdArgs) -> CniResult<()> {
    info!("cmd_args: {:?}", args);
    let mut stdin_data = Vec::new();
    stdin().read_to_end(&mut stdin_data)?;
    let mut net_conf: NetConf = serde_json::from_slice(&stdin_data)?;
    info!("net_config: {:#?}", net_conf);

    if net_conf.is_default_gw.unwrap_or_default() {
        net_conf.is_gw = Some(true);
    }
    let (br_link, br_interface) = setup_bridge(&net_conf)?;
    let netns = Netns::get_from_path(args.netns.as_ref())?.ok_or(anyhow!("netns not found"))?;

    let current_ns = Netns::get()?;
    let (host_interface, container_interface) = setup_veth(
        &current_ns,
        &netns,
        &br_link,
        &args.if_name,
        net_conf.mtu.unwrap_or(1500),
        false,
        false,
        0,
        vec![],
        false,
        "",
    )?;
    info!("host_interface: {:?}", host_interface);
    info!("container_interface: {:?}", container_interface);

    let mut bridge_result = ExecResult {
        cni_version: Some("1.0.0".to_string()),
        interfaces: Some(vec![br_interface, host_interface, container_interface]),
        ips: None,
        routes: None,
        dns: None,
    };

    {
        let mut ipam_result: ExecResult = invoke::delegate_add(&net_conf.ipam.plugin, &stdin_data)?;
        bridge_result.ips = ipam_result.ips;
        bridge_result.routes = ipam_result.routes;
        bridge_result.dns = ipam_result.dns;
    }

    let gateway_infos = calc_gateway(&mut bridge_result, &net_conf)?;
    info!("gateway_infos: {:?}", gateway_infos);

    info!("bridge_result: {:?}", bridge_result);

    netns_ng::exec_netns!(current_ns, &netns, result, || {
        ipam::config_interface(&args.if_name, &bridge_result)
    });
    result?;

    if net_conf.is_gw.unwrap_or(false) {
        for gw_info in &gateway_infos {
            for gw in &gw_info.gws {
                // set gateway ip to bridge
                ensure_addr(&br_link, gw, net_conf.force_address.unwrap_or_default())?;
            }

            if !gw_info.gws.is_empty() {
                enable_ip_forward(gw_info.family)?;
            }
        }
    }
    if net_conf.ip_masq.unwrap_or_default() {
        let chain_name = utils::format_chain_name(&net_conf.name, &args.container_id);
        for ip in bridge_result.ips.as_deref().unwrap_or_default() {
            ip::setup_ip_masq(&ip.address, &chain_name)?;
        }
    }

    let _ = stdout().write_fmt(format_args!(
        "{}",
        serde_json::to_string_pretty(&bridge_result)?
    ));
    Ok(())
}

fn enable_ip_forward(family: Family) -> CniResult<()> {
    match family {
        FAMILY_V4 => ip::enable_ipv4_forward(),
        FAMILY_V6 => ip::enable_ipv6_forward(),
        _ => bail!("not support family: {}", family),
    }
}

fn ensure_addr(br: &Link, ip: &IpNetwork, force_address: bool) -> CniResult<()> {
    let family = match ip {
        IpNetwork::V4(_) => FAMILY_V4,
        IpNetwork::V6(_) => FAMILY_V6,
    };
    let addrs = netlink_ng::addr_list(br.as_index(), family)?;
    for addr_item in &addrs {
        if addr_item.ipnet.ip() == ip.ip() {
            return Ok(());
        }
        // Multiple IPv6 addresses are allowed on the bridge if the
        // corresponding subnets do not overlap. For IPv4 or for
        // overlapping IPv6 subnets, reconfigure the IP address if
        // forceAddress is true, otherwise throw an error.
        if family == FAMILY_V4
            || addr_item.ipnet.contains(ip.ip())
            || ip.contains(addr_item.ipnet.ip())
        {
            if !force_address {
                bail!(
                    "{} already has an IP address different from {}",
                    br.attrs().name,
                    ip
                );
            }
            netlink_ng::addr_del(br.as_index(), addr_item)?;
        }
    }
    let addr = netlink_ng::Addr {
        ipnet: ip.clone(),
        ..Default::default()
    };
    info!("add addr to br, addr: {:?}", addr);
    netlink_ng::addr_add(br.as_index(), &addr)?;
    // todo set bridge mac addr

    Ok(())
}

fn calc_gateway(ipam_result: &mut ExecResult, net_conf: &NetConf) -> CniResult<Vec<GatewayInfo>> {
    let mut ips = ipam_result
        .ips
        .as_deref_mut()
        .ok_or(anyhow!("IPAM plugin returned missing IP config"))?;

    let mut gws = Vec::new();
    let is_default_gw = net_conf.is_default_gw.clone().unwrap_or(false);
    let is_gw = net_conf.is_gw.clone().unwrap_or(false);
    for ip in ips.iter_mut() {
        // index 1 is lo, index2 is eth0
        ip.interface = Some(2);
        if ip.gateway.is_none() && is_gw {
            ip.gateway = ip::next_ip(&ip.address.ip());
        }
        let mut gw_info = GatewayInfo::default();
        if ip.address.is_ipv4() {
            gw_info.family = FAMILY_V4;
        } else if ip.address.is_ipv6() {
            gw_info.family = FAMILY_V6;
        }

        // Add a default route for this family using the current
        // gateway address if necessary.

        if is_default_gw {
            if let Some(routes) = ipam_result.routes.as_deref() {
                for route in routes {
                    if route.gw.is_some() && route.dst.ip().is_unspecified() {
                        gw_info.default_route_found = true;
                        break;
                    }
                }
            }
            if !gw_info.default_route_found {
                let route = Route {
                    dst: IpNetwork::V4(Ipv4Network::new(Ipv4Addr::UNSPECIFIED, 0).unwrap()),
                    gw: ip.gateway,
                };
                ipam_result.routes.get_or_insert_with(Vec::new).push(route);
            }
        }
        if is_gw {
            let gw = IpNetwork::with_netmask(ip.gateway.clone().unwrap(), ip.address.mask())?;
            // let gw = match ip.address {
            //     IpNetwork::V4(ipv4_network) => {
            //         // let mask = ipv4_network.mask();
            //         // let ip = ipv4_network.ip();
            //         let ip_network = wrap_err!(Ipv4Network::with_netmask(ip.gateway.clone().unwrap(), mask), "not valid net mask".into())?;
            //
            //         IpNetwork::V4(ip_network)
            //     }
            //     IpNetwork::V6(_) => {
            //         todo!()
            //     }
            // };
            gw_info.gws.push(gw);
            gws.push(gw_info);
        }
    }

    Ok(gws)
}

#[derive(Debug, Default)]
pub struct GatewayInfo {
    pub gws: Vec<IpNetwork>,
    pub family: Family,
    pub default_route_found: bool,
}

fn setup_veth(
    host_ns: &Netns,
    netns: &Netns,
    br: &Link,
    if_name: &str,
    mtu: u32,
    hairpin_mode: bool,
    promisc_mode: bool,
    vlan_id: u16,
    vlans: Vec<u32>,
    preserve_default_vlan: bool,
    mac: &str,
) -> CniResult<(Interface, Interface)> {
    // let host_ns = Netns::get()?;

    info!("netns: {:?}", netns.unique_id());
    info!("host_ns: {:?}", host_ns.unique_id());

    netns_ng::exec_netns!(
        host_ns,
        netns,
        result,
        || -> anyhow::Result<(Interface, Interface)> {
            let cur_ns = Netns::get()?;
            anyhow::ensure!(&cur_ns == netns, "netns not match");

            let (host_veth, container_veth) =
                ip::setup_veth(if_name, "", mtu, mac, &host_ns, &netns)?;
            Ok((
                Interface {
                    name: host_veth.link_attrs.name.clone(),
                    ..Default::default()
                },
                Interface {
                    name: container_veth.link_attrs.name.clone(),
                    mac: container_veth
                        .link_attrs
                        .hardware_addr
                        .map(|it| MacAddr::try_from(it.as_slice()))
                        .transpose()?,
                    sandbox: Some(netns.path().unwrap_or_default()),
                },
            ))
        }
    );

    let (mut host_interface, container_interface) = result?;

    info!(">>>>host_interface: {:?}", host_interface);
    info!(">>>>container_interface: {:?}", container_interface);

    let host_veth =
        netlink_ng::link_by_name(&host_interface.name)?.ok_or(anyhow!("veth not found"))?;
    netlink_ng::link_set_master(&host_veth, br)?;
    let host_mac = host_veth
        .attrs()
        .hardware_addr
        .as_deref()
        .map(|it| MacAddr::try_from(it))
        .transpose()?
        .ok_or(anyhow!("veth mac not found"))?;
    host_interface.mac = Some(host_mac);
    Ok((host_interface, container_interface))
}

fn setup_bridge(net_conf: &NetConf) -> CniResult<(Link, Interface)> {
    let vlan_filtering = net_conf.vlan.is_some() || net_conf.vlan_trunk.is_some();
    let br_name = net_conf.br_name.as_deref().unwrap_or("cni0");
    let mtu = net_conf.mtu.clone().unwrap_or(0);

    let br = ensure_bridge(
        br_name,
        mtu,
        net_conf.promisc_mode.unwrap_or_default(),
        vlan_filtering,
    )?;
    let br_mac = {
        match &br.attrs().hardware_addr {
            None => MacAddr::default(),
            Some(addr) => MacAddr::try_from(addr.as_slice())?,
        }
    };

    let interface = Interface {
        name: br.attrs().name.clone(),
        mac: Some(br_mac),
        ..Default::default()
    };

    Ok((br, interface))
}

pub fn ensure_bridge(
    br_name: &str,
    mtu: u32,
    promisc_mode: bool,
    vlan_filtering: bool,
) -> CniResult<Link> {
    let br = Link {
        link_attrs: LinkAttrs {
            mtu,
            name: br_name.to_string(),
            ..Default::default()
        },
        link_kind: LinkKind::Bridge(Bridge {
            vlan_filtering: Some(vlan_filtering),
            ..Default::default()
        }),
    };

    if let Err(e) = netlink_ng::link_add(&br) {
        if !is_already_exists_error(&e) {
            bail!("link add failed: {:?}", e);
        }
    };
    if promisc_mode {
        let link_index = br_name.try_as_index()?.ok_or(anyhow!("bridge not found"))?;
        netlink_ng::set_promisc_on(link_index)?;
    }

    let br = bridge_by_name(br_name)?.ok_or(anyhow!("bridge not found"))?;

    // we want to own the routes for this interface
    utils::sysctl_set(format!("net/ipv6/conf/{}/accept_ra", br_name).as_str(), "1")?;

    netlink_ng::link_set_up(br.as_index())?;
    Ok(br)
}

fn bridge_by_name(br_name: &str) -> CniResult<Option<Link>> {
    let link = netlink_ng::link_by_name(br_name)?;
    match link {
        None => Ok(None),
        Some(link) => {
            let link_kind = &link.link_kind;
            if !matches!(link_kind, LinkKind::Bridge(_)) {
                bail!("link {} is not a bridge", br_name);
            }
            Ok(Some(link))
        }
    }
}
