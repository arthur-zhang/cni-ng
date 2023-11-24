use serde::{Deserialize, Serialize};

use cni_core::types::IPAMConfig;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetConf {
    pub cni_version: String,
    pub name: String,
    #[serde(rename = "type")]
    pub plugin: String,
    #[serde(rename = "bridge", default, skip_serializing_if = "Option::is_none")]
    pub br_name: Option<String>,
    #[serde(rename = "isGateway", default, skip_serializing_if = "Option::is_none")]
    pub is_gw: Option<bool>,
    #[serde(
        rename = "isDefaultGateway",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub is_default_gw: Option<bool>,
    #[serde(
        rename = "forceAddress",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub force_address: Option<bool>,
    #[serde(rename = "ipMasq", default, skip_serializing_if = "Option::is_none")]
    pub ip_masq: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mtu: Option<u32>,
    #[serde(
        rename = "hairpinMode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub hairpin_mode: Option<bool>,
    #[serde(default)]
    pub ipam: IPAMConfig,
    #[serde(
        rename = "promiscMode",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub promisc_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vlan: Option<i32>,
    #[serde(rename = "vlanTrunk", default, skip_serializing_if = "Option::is_none")]
    pub vlan_trunk: Option<Vec<VlanTrunk>>,
    #[serde(
        rename = "preserveDefaultVlan",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub preserve_default_vlan: Option<bool>,
    #[serde(
        rename = "macspoofchk",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub mac_spoof_chk: Option<bool>,
    #[serde(rename = "enabledad", default, skip_serializing_if = "Option::is_none")]
    pub enable_dad: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macspoofchk: Option<bool>,
    // #[serde(default, skip_serializing_if = "Option::is_none")]
    // pub mac: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlanTrunk {
    #[serde(rename = "minID")]
    min_id: Option<i32>,
    #[serde(rename = "maxID")]
    max_id: Option<i32>,
    #[serde(rename = "id")]
    id: Option<i32>,
}

// pub struct RuntimeConfig {}
