use serde::{Deserialize, Serialize};

use cni_core::types::{IPAMArgs, IPAMConfig};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetConf {
    pub cni_version: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipam: Option<IPAMConfig>,
    #[serde(
        default,
        rename = "runtimeConfig",
        skip_serializing_if = "Option::is_none"
    )]
    pub runtime: Option<RuntimeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<IPAMArgs>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    ips: Option<Vec<String>>,
}
