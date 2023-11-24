use std::net::Ipv4Addr;
use std::str::FromStr;

use ipnetwork::IpNetwork;

use cni_core::prelude::CniResult;
use cni_core::wrap_err;

const IP_V4_MULTICAST_NET: &'static str = "224.0.0.0/4";
const IP_V6_MULTICAST_NET: &'static str = "ff00::/8";

// Chain POSTROUTING (policy ACCEPT)
// target     prot opt source               destination
// cni-012    all  --  192.168.0.1          0.0.0.0/0
//
// Chain cni-012 (1 references)
// target     prot opt source               destination
// ACCEPT     all  --  0.0.0.0/0            192.168.0.0/24
// MASQUERADE  all  --  0.0.0.0/0           !224.0.0.0/4
pub fn setup_ip_masq(ip: &IpNetwork, chain_name: &str) -> CniResult<()> {
    let ip_addr = ip.ip();

    let multicast_net = if ip.is_ipv4() {
        IP_V4_MULTICAST_NET
    } else {
        IP_V6_MULTICAST_NET
    };

    let ipt = iptables::new(ip.is_ipv6()).unwrap();
    let chains = wrap_err!(ipt.list_chains("nat"))?;
    let exists = chains.iter().any(|c| c == chain_name);
    if !exists {
        wrap_err!(ipt.new_chain("nat", chain_name))?;
    }

    // Packets to this network should not be touched
    let rule = format!("-d {} -j ACCEPT", ip);
    // Chain cni-012 (1 references)
    // target     prot opt source               destination
    // ACCEPT     all  --  0.0.0.0/0            192.168.0.0/24
    wrap_err!(ipt.append_unique("nat", chain_name, &rule))?;

    // Don't masquerade multicast - pods should be able to talk to other pods
    // on the local network via multicast.
    let rule = format!("! -d {} -j MASQUERADE", multicast_net).to_string();
    // Chain cni-012 (1 references)
    // target      prot opt source              destination
    // MASQUERADE  all  --  0.0.0.0/0           !224.0.0.0/4
    wrap_err!(ipt.append_unique("nat", chain_name, &rule))?;

    // Packets from the specific IP of this network will hit the chain
    let rule = format!("-s {} -j {}", ip_addr, chain_name);
    // Chain POSTROUTING (policy ACCEPT)
    // target     prot opt source               destination
    // cni-012    all  --  192.168.0.1          0.0.0.0/0
    wrap_err!(ipt.append_unique("nat", "POSTROUTING", &rule))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn test_setup_ip_masq() {
        let ip = ipnetwork::IpNetwork::V4(
            ipnetwork::Ipv4Network::new(Ipv4Addr::new(192, 168, 0, 1), 24).unwrap(),
        );
        let chain = "cni-012";
        let list = setup_ip_masq(&ip, chain).unwrap();
    }
}
