use std::fmt::Display;
use std::net::IpAddr;

use anyhow::bail;

use crate::range::Range;

pub type RangeSet = Vec<Range>;

pub trait RangeSetExt {
    fn canonicalize(&mut self) -> anyhow::Result<()>;
    fn contains_ip(&self, ip_addr: IpAddr) -> bool;
    fn overlap(&self, other: &RangeSet) -> bool;
    fn to_string(&self) -> String;
}

impl RangeSetExt for RangeSet {
    fn canonicalize(&mut self) -> anyhow::Result<()> {
        if self.is_empty() {
            bail!("empty range set")
        }

        for range in self.iter_mut() {
            range.canonicalize()?;
        }

        // Make sure none of the ranges in the set overlap
        let n = self.len();
        for i in 0..n {
            for j in i + 1..n {
                if self[i].overlap(&self[j]) {
                    bail!("subnets {} and {} overlap", self[i], self[j])
                }
            }
        }
        Ok(())
    }
    fn contains_ip(&self, ip_addr: IpAddr) -> bool {
        self.iter().any(|range| range.contains(ip_addr))
    }

    // Overlaps returns true if any ranges in any set overlap with this one
    fn overlap(&self, other: &RangeSet) -> bool {
        for r1 in self.iter() {
            for r2 in other.iter() {
                if r1.overlap(r2) {
                    return true;
                }
            }
        }
        false
    }

    fn to_string(&self) -> String {
        self.iter()
            .map(|it| it.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let mut p = vec![
            Range {
                subnet: "192.168.0.0/24".parse().unwrap(),
                ..Default::default()
            },
            Range {
                subnet: "172.16.1.0/24".parse().unwrap(),
                ..Default::default()
            },
        ];
        p.canonicalize().unwrap();
        assert!(p.contains_ip("192.168.0.55".parse().unwrap()))
    }

    #[test]
    fn test2() {
        let mut p = vec![
            Range {
                subnet: "192.168.0.0/20".parse().unwrap(),
                ..Default::default()
            },
            Range {
                subnet: "192.168.2.0/24".parse().unwrap(),
                ..Default::default()
            },
        ];
        let result = p.canonicalize();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(
            err.to_string(),
            "subnets 192.168.0.1-192.168.15.254 and 192.168.2.1-192.168.2.254 overlap"
        );
    }

    #[test]
    fn test3() {
        let mut p1 = vec![Range {
            subnet: "192.168.0.0/20".parse().unwrap(),
            ..Default::default()
        }];
        let mut p2 = vec![Range {
            subnet: "192.168.2.0/24".parse().unwrap(),
            ..Default::default()
        }];
        p1.canonicalize().unwrap();
        p2.canonicalize().unwrap();
        assert!(p1.overlap(&p2));
        assert!(p2.overlap(&p1));
    }
}
