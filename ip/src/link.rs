use anyhow::{anyhow, bail};
use log::info;
use netlink_ng::nl_type::Veth;
use netlink_ng::{Link, LinkAttrs, LinkId, LinkKind, Namespace};
use netns_ng::Netns;
use rand::random;

// Call setup_veth from inside the container netns.
pub fn setup_veth(
    container_veth_name: &str,
    host_veth_name: &str,
    mtu: u32,
    container_veth_mac: &str,
    host_ns: &Netns,
    container_ns: &Netns,
) -> anyhow::Result<(Link, Link)> {
    let current_ns = Netns::get()?;
    anyhow::ensure!(&current_ns == container_ns, "netns not match");

    let (host_veth_name, container_veth) = make_veth(
        container_veth_name,
        host_veth_name,
        mtu,
        container_veth_mac,
        host_ns,
        container_ns,
    )?;
    // save a handle to current network namespace

    // enter host_ns and set host veth up, then return to container ns
    netns_ng::exec_netns!(&current_ns, &host_ns, result, || {
        let host_veth =
            netlink_ng::link_by_name(&host_veth_name)?.ok_or(anyhow!("veth not found"))?;
        netlink_ng::link_set_up(&host_veth)?;
        Ok(host_veth)
    });
    let host_veth: Result<Link, anyhow::Error> = result;

    Ok((host_veth?, container_veth))
}

fn make_veth(
    container_veth_name: &str,
    host_veth_name: &str,
    mtu: u32,
    container_veth_mac: &str,
    host_ns: &Netns,
    container_ns: &Netns,
) -> anyhow::Result<(String, Link)> {
    let cur_ns = Netns::get()?;
    anyhow::ensure!(&cur_ns == container_ns, "netns not match");
    // sleep(std::time::Duration::from_secs(10));

    let mut peer_name = host_veth_name.to_string();
    for i in 0..10 {
        if host_veth_name.is_empty() {
            peer_name = random_veth_name();
        }
        let res = make_veth_pair(
            container_veth_name,
            &peer_name,
            mtu,
            container_veth_mac,
            host_ns,
            container_ns,
        );
        match res {
            Ok(res) => {
                return Ok((peer_name, res));
            }
            Err(ref e) => {
                bail!(
                    "make_veth_pair failed: {:?}, peer:{}, container:{}",
                    res,
                    peer_name,
                    container_veth_name
                );
            }
        }
    }
    bail!("failed to create veth pair");
}

// make_veth_pair is called from within the container's network namespace
fn make_veth_pair(
    container_veth_name: &str,
    host_veth_name: &str,
    mtu: u32,
    container_veth_mac: &str,
    host_ns: &Netns,
    container_ns: &Netns,
) -> anyhow::Result<Link> {
    let cur_ns = Netns::get()?;
    anyhow::ensure!(&cur_ns == container_ns, "netns not match");

    info!(
        "make_veth_pair: container_veth_name: {}, host_veth_name: {}",
        container_veth_name, host_veth_name
    );
    let link = Link {
        link_attrs: LinkAttrs {
            name: container_veth_name.to_string(),
            // mtu,
            ..Default::default()
        },
        link_kind: LinkKind::Veth(Veth {
            peer_name: host_veth_name.to_string(),
            peer_namespace: Namespace::NsFd(host_ns.fd() as u32),
            ..Default::default()
        }),
    };
    // todo process mac
    netlink_ng::link_add(&link)?;

    let cur_ns = Netns::get()?;
    anyhow::ensure!(&cur_ns == container_ns, "netns not match");

    info!("link_add success");
    let link = netlink_ng::link_by_name(container_veth_name)?.ok_or(anyhow!("veth not found"));
    info!("link by name result: {:?}", link);
    match link {
        Ok(link) => Ok(link),
        Err(e) => {
            netlink_ng::link_del(LinkId::Name(container_veth_name))?;
            return Err(e);
        }
    }
}

fn random_veth_name() -> String {
    let entropy: [u8; 4] = random();
    format!(
        "veth{:02x}{:02x}{:02x}{:02x}",
        entropy[0], entropy[1], entropy[2], entropy[3]
    )
}
