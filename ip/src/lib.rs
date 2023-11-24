use std::net::{IpAddr, Ipv4Addr};

use cni_core::prelude::CniResult;
pub use ip_masq::*;
pub use link::*;

mod ip_masq;
mod link;

pub fn next_ip(ip: &IpAddr) -> Option<IpAddr> {
    match ip {
        IpAddr::V4(ipv4) => {
            let ip = ipv4.octets();
            let ip_num = u32::from_be_bytes(ip);
            let (ip_num, overflow) = ip_num.overflowing_add(1);
            if overflow {
                return None;
            }
            Some(IpAddr::V4(Ipv4Addr::from(ip_num.to_be_bytes())))
        }
        IpAddr::V6(ipv6) => {
            todo!()
        }
    }
}

pub fn enable_ipv4_forward() -> CniResult<()> {
    utils::sysctl_set("net/ipv4/ip_forward", "1")
}

pub fn enable_ipv6_forward() -> CniResult<()> {
    utils::sysctl_set("net/ipv6/conf/all/forwarding", "1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_ip() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1));
        // let ip = IpAddr::V4(Ipv4Addr::new(255, 255, 255, 255));
        let next_ip = next_ip(&ip);
        assert_eq!(next_ip, Some(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 2))));
    }
}
