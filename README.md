# Rust CNI (Container Network Interface)

Rust CNI is an implementation of the Container Network Interface (CNI) in Rust. CNI provides a general solution for network connectivity for containers.

The project is currently in its early stages of development, and we welcome contributions from the community. If you are interested in contributing.


## Project Introduction

Rust CNI aims to provide a more performant and reliable alternative to the existing CNI implementations. By leveraging the features and strengths of Rust, we hope to address some of the common issues faced by other implementations, such as memory safety and concurrency.

## About netlink

I'm current writing a golang style Netlink library for rust: [netlink-ng](https://github.com/container-ng/netlink-ng). Much easier to use, believe me.

## Features

- [ ] Fast and efficient networking for containers
- [ ] Memory safe and concurrent handling of network connections
- [ ] Easy to integrate with existing container orchestration tools
- [ ] Fully compatible with the CNI specification

## Roadmap

Here is a list of CNI plugins that we plan to support. This list is based on the plugins listed at [CNI plugins](https://www.cni.dev/plugins/current/).

- [x] bridge
- [x] flannel
- [ ] portmap
- [ ] ipvlan
- [ ] macvlan
- [ ] ptp
- [ ] vlan
- [ ] loopback
- [ ] tuning
- [ ] bandwidth
- [ ] sbr
- [ ] firewall

## IPAM
- [x] static
- [x] host-local
- [ ] dhcp

## License

Rust CNI is released under the [MIT License](LICENSE).