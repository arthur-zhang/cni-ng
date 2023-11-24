use anyhow::bail;
use std::collections::HashMap;
use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::str::FromStr;

use ipnetwork::IpNetwork;
use macaddr::{MacAddr6, ParseError};
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessReply {
    /// The CNI version of the plugin input config.
    pub cni_version: String,

    /// The list of all interfaces created by this plugin.
    ///
    /// If `prev_result` was included in the input config and had interfaces,
    /// they need to be carried on through into this list.
    #[serde(default)]
    pub interfaces: Vec<Interface>,

    /// The list of all IPs assigned by this plugin.
    ///
    /// If `prev_result` was included in the input config and had IPs,
    /// they need to be carried on through into this list.
    #[serde(default)]
    pub ips: Vec<Ip>,

    /// The list of all routes created by this plugin.
    ///
    /// If `prev_result` was included in the input config and had routes,
    /// they need to be carried on through into this list.
    #[serde(default)]
    pub routes: Vec<Route>,

    /// Final DNS configuration for the namespace.
    pub dns: Dns,

    /// Custom reply fields.
    ///
    /// Note that these are off-spec and may be discarded by libcni.
    #[serde(flatten)]
    pub specific: HashMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ip {
    /// The IP address.
    pub address: IpNetwork,

    /// The default gateway for this subnet, if one exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway: Option<IpAddr>,

    /// The interface this IP is for.
    ///
    /// This must be the index into the `interfaces` list on the parent success
    /// reply structure. It should be `None` for IPAM success replies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface: Option<usize>, // None for ipam
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Interface {
    /// The name of the interface.
    pub name: String,

    /// The hardware address of the interface (if applicable).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<MacAddr>,

    /// The path to the namespace the interface is in.
    ///
    /// This should be the value passed by `CNI_NETNS`.
    ///
    /// If the interface is on the host, this should be set to an empty string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    /// The destination of the route.
    pub dst: IpNetwork,

    /// The next hop address.
    ///
    /// If unset, a value in `gateway` in the `ips` array may be used by the
    /// runtime, but this is not mandated and is left to its discretion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gw: Option<IpAddr>,
}

#[derive(Debug, Default, Hash, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
pub struct MacAddr(macaddr::MacAddr6);

impl From<MacAddr6> for MacAddr {
    fn from(m: MacAddr6) -> Self {
        Self(m)
    }
}

impl From<MacAddr> for MacAddr6 {
    fn from(m: MacAddr) -> Self {
        m.0
    }
}

impl fmt::Display for MacAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for MacAddr {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        MacAddr6::from_str(s).map(Self)
    }
}

impl From<[u8; 6]> for MacAddr {
    fn from(bytes: [u8; 6]) -> Self {
        Self(MacAddr6::from(bytes))
    }
}

impl TryFrom<&[u8]> for MacAddr {
    type Error = anyhow::Error;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Ok(Self::default());
        }
        if value.len() != 6 {
            bail!("invalid mac address");
        }
        let mut bytes = [0u8; 6];
        bytes.copy_from_slice(value);
        Ok(Self::from(bytes))
    }
}

impl Serialize for MacAddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MacAddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let j = String::deserialize(deserializer)?;
        Self::from_str(&j).map_err(Error::custom)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dns {
    /// List of DNS nameservers this network is aware of.
    ///
    /// The list is priority-ordered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nameservers: Vec<IpAddr>,

    /// The local domain used for short hostname lookups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,

    /// List of search domains for short hostname lookups.
    ///
    /// This effectively supersedes the `domain` field and will be preferred
    /// over it by most resolvers.
    ///
    /// The list is priority-ordered.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,

    /// List of options to be passed to the resolver.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IPAMArgs {
    ips: Vec<String>,
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IPAMConfig {
    #[serde(rename = "type")]
    pub plugin: String,
    pub routes: Option<Vec<Route>>,
    pub addresses: Option<Vec<Ip>>,
    pub dns: Option<Dns>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExecResult {
    #[serde(rename = "cniVersion")]
    pub cni_version: Option<String>,
    #[serde(rename = "interfaces", skip_serializing_if = "Option::is_none")]
    pub interfaces: Option<Vec<Interface>>,
    #[serde(rename = "ips")]
    pub ips: Option<Vec<Ip>>,
    #[serde(rename = "routes")]
    pub routes: Option<Vec<Route>>,
    #[serde(rename = "dns")]
    pub dns: Option<Dns>,
}
