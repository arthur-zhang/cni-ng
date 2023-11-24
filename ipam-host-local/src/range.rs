use std::fmt::{Display, Formatter};
use std::net::IpAddr;

use anyhow::bail;
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};

use crate::allocator::{last_ip, next_ip};

#[derive(Serialize, Deserialize, Debug, PartialOrd, PartialEq)]
pub struct Range {
    // IP inside of “subnet” from which to start allocating addresses.
    // Defaults to “.2” IP inside of the “subnet” block.
    #[serde(rename = "rangeStart")]
    pub range_start: Option<IpAddr>,

    // IP inside of “subnet” with which to end allocating addresses.
    // Defaults to “.254” IP inside of the “subnet” block for ipv4, “.255” for IPv6
    #[serde(rename = "rangeEnd")]
    pub range_end: Option<IpAddr>,

    // CIDR block to allocate out of
    #[serde(rename = "subnet")]
    pub subnet: ipnetwork::IpNetwork,

    // IP inside of “subnet” to designate as the gateway.
    // Defaults to “.1” IP inside of the “subnet” block.
    #[serde(rename = "gateway")]
    pub gateway: Option<IpAddr>,
}

impl Default for Range {
    fn default() -> Self {
        Self {
            range_start: None,
            range_end: None,
            subnet: IpNetwork::new(IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)), 0).unwrap(),
            gateway: None,
        }
    }
}

impl Range {
    // Canonicalize takes a given range and ensures that all information is consistent,
    // filling out Start, End, and Gateway with sane values if missing
    pub fn canonicalize(&mut self) -> anyhow::Result<()> {
        let prefix = self.subnet.prefix();
        if prefix >= 31 {
            bail!("Network {} too small to allocate from", self.subnet);
        }

        if self.subnet.ip() != self.subnet.network() {
            bail!("Network has host bits set. For a subnet mask of length {} the network address is {}",self.subnet.prefix(), self.subnet.network());
        }

        // If the gateway is nil, claim .1
        if self.gateway.is_none() {
            self.gateway = next_ip(&self.subnet.ip());
        }
        // RangeStart: If specified, make sure it's sane (inside the subnet),
        // otherwise use the first free IP (i.e. .1) - this will conflict with the
        // gateway but we skip it in the iterator
        if let Some(range_start) = self.range_start {
            if !self.contains(range_start) {
                bail!("RangeStart {} not in network {}", range_start, self.subnet);
            }
        } else {
            self.range_start = next_ip(&self.subnet.ip());
        }
        // RangeEnd: If specified, verify sanity. Otherwise, add a sensible default
        // (e.g. for a /24: .254 if IPv4, ::255 if IPv6)
        if let Some(range_end) = self.range_end {
            if !self.contains(range_end) {
                bail!("RangeEnd {} not in network {}", range_end, self.subnet);
            }
        } else {
            self.range_end = Some(last_ip(&self.subnet));
        }
        Ok(())
    }
    pub fn contains(&self, addr: IpAddr) -> bool {
        // Not in network
        if !self.subnet.contains(addr) {
            return false;
        }
        if let Some(range_start) = &self.range_start {
            if addr < *range_start {
                return false;
            }
        }
        if let Some(range_end) = &self.range_end {
            if addr > *range_end {
                return false;
            }
        }
        true
    }

    pub fn overlap(&self, other: &Range) -> bool {
        // should not happen
        if other.range_start.is_none()
            || other.range_end.is_none()
            || self.range_start.is_none()
            || self.range_end.is_none()
        {
            return false;
        }
        // 	different families
        if (self.subnet.is_ipv4() && other.subnet.is_ipv6())
            || (self.subnet.is_ipv6() && other.subnet.is_ipv4())
        {
            return false;
        }

        self.contains(other.range_start.unwrap())
            || self.contains(other.range_end.unwrap())
            || other.contains(self.range_start.unwrap())
            || other.contains(self.range_end.unwrap())
    }
}

impl Display for Range {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let range_start = self
            .range_start
            .map(|it| it.to_string())
            .unwrap_or("<nil>".into());
        let range_end = self
            .range_end
            .map(|it| it.to_string())
            .unwrap_or("<nil>".into());
        write!(f, "{}-{}", range_start, range_end)
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use ipnetwork::IpNetwork;

    use super::*;

    #[test]
    fn test_range() {
        let mut r = Range {
            range_start: None,
            range_end: None,
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: None,
        };
        r.canonicalize().unwrap();
        println!("{:?}", r);
        assert_eq!(
            r,
            Range {
                range_start: Some("192.0.2.1".parse().unwrap()),
                range_end: Some("192.0.2.254".parse().unwrap()),
                subnet: "192.0.2.0/24".try_into().unwrap(),
                gateway: Some("192.0.2.1".parse().unwrap()),
            }
        );
    }

    #[test]
    fn test_range2() {
        let mut r = Range {
            range_start: None,
            range_end: None,
            subnet: "192.0.2.0/25".try_into().unwrap(),
            gateway: None,
        };
        r.canonicalize().unwrap();

        println!("{:?}", r);
        assert_eq!(
            r,
            Range {
                range_start: Some("192.0.2.1".parse().unwrap()),
                range_end: Some("192.0.2.126".parse().unwrap()),
                subnet: "192.0.2.0/25".try_into().unwrap(),
                gateway: Some("192.0.2.1".parse().unwrap()),
            }
        );
    }

    #[test]
    fn test_range3() {
        let mut r = Range {
            range_start: None,
            range_end: None,
            subnet: "192.0.2.12/24".try_into().unwrap(),
            gateway: None,
        };
        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Network has host bits set. For a subnet mask of length 24 the network address is 192.0.2.0");
    }

    #[test]
    fn test_range5() {
        let mut r = Range {
            range_start: None,
            range_end: None,
            subnet: "192.168.127.0/23".try_into().unwrap(),
            gateway: None,
        };
        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "Network has host bits set. For a subnet mask of length 23 the network address is 192.168.126.0");
    }

    #[test]
    fn test_range6() {
        let mut r = Range {
            range_start: None,
            range_end: None,
            subnet: "192.0.2.0/31".try_into().unwrap(),
            gateway: None,
        };
        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "Network 192.0.2.0/31 too small to allocate from"
        );
    }

    #[test]
    fn test_range7() {
        let mut r = Range {
            range_start: Some("192.0.3.0".parse().unwrap()),
            range_end: None,
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: None,
        };
        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "RangeStart 192.0.3.0 not in network 192.0.2.0/24"
        );

        let mut r = Range {
            range_start: None,
            range_end: Some("192.0.4.0".parse().unwrap()),
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: None,
        };
        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "RangeEnd 192.0.4.0 not in network 192.0.2.0/24"
        );

        let mut r = Range {
            range_start: Some("192.0.2.50".parse().unwrap()),
            range_end: Some("192.0.2.40".parse().unwrap()),
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: None,
        };

        let result = r.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "RangeStart 192.0.2.50 not in network 192.0.2.0/24"
        );
    }

    #[test]
    fn test_range8() {
        let mut r = Range {
            range_start: Some("192.0.2.40".parse().unwrap()),
            range_end: Some("192.0.2.50".parse().unwrap()),
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: Some("192.0.2.254".parse().unwrap()),
        };

        r.canonicalize().unwrap();
        assert_eq!(
            r,
            Range {
                range_start: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 40))),
                range_end: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 50))),
                subnet: IpNetwork::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 0)), 24).unwrap(),
                gateway: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 254))),
            }
        )
    }

    #[test]
    fn test_range9() {
        let mut r = Range {
            range_start: Some("192.0.2.40".parse().unwrap()),
            range_end: Some("192.0.2.50".parse().unwrap()),
            subnet: "192.0.2.0/24".try_into().unwrap(),
            gateway: Some("192.0.2.254".parse().unwrap()),
        };

        r.canonicalize().unwrap();
        assert!(!r.contains("192.0.3.0".parse().unwrap()));
        assert!(!r.contains("192.0.2.39".parse().unwrap()));
        assert!(r.contains("192.0.2.40".parse().unwrap()));
        assert!(r.contains("192.0.2.50".parse().unwrap()));
        assert!(!r.contains("192.0.2.51".parse().unwrap()));
    }

    #[test]
    fn test_range10() {
        {
            let mut r1 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                ..Default::default()
            };
            r1.canonicalize().unwrap();
            let mut r2 = Range {
                subnet: "10.0.1.0/24".try_into().unwrap(),
                ..Default::default()
            };
            r2.canonicalize().unwrap();
            assert!(!r1.overlap(&r2));
        }
        {
            let mut r1 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                ..Default::default()
            };
            r1.canonicalize().unwrap();
            let mut r2 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                ..Default::default()
            };
            r2.canonicalize().unwrap();
            assert!(r1.overlap(&r2));
        }
        {
            let mut r1 = Range {
                subnet: "10.0.0.0/20".try_into().unwrap(),
                ..Default::default()
            };
            r1.canonicalize().unwrap();
            let mut r2 = Range {
                subnet: "10.0.1.0/24".try_into().unwrap(),
                ..Default::default()
            };
            r2.canonicalize().unwrap();
            assert!(r1.overlap(&r2));
        }

        {
            let mut r1 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                range_end: Some("10.0.0.127".parse().unwrap()),
                ..Default::default()
            };
            r1.canonicalize().unwrap();
            let mut r2 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                range_start: Some("10.0.0.128".parse().unwrap()),
                ..Default::default()
            };
            r2.canonicalize().unwrap();
            assert!(!r1.overlap(&r2));
        }

        {
            let mut r1 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                range_end: Some("10.0.0.127".parse().unwrap()),
                ..Default::default()
            };
            r1.canonicalize().unwrap();
            let mut r2 = Range {
                subnet: "10.0.0.0/24".try_into().unwrap(),
                range_start: Some("10.0.0.127".parse().unwrap()),
                ..Default::default()
            };
            r2.canonicalize().unwrap();
            assert!(r1.overlap(&r2));
        }
    }
}
