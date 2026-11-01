---
title: "IPv4 Addressing and Subnetting"
order: 23
level: IA
topic: ipv4
lab: office-l3
---

An IPv4 address is 32 bits written as four decimal octets. What decides whether two addresses share a subnet is not the address itself but the **mask (prefix length)**.

## Three numbers to remember

- **Network address**: all host bits zero, identifies the subnet;
- **Broadcast address**: all host bits one, delivered to every host in the subnet;
- **Usable hosts**: 2^host bits − 2 (minus network and broadcast).

Common prefixes:

```
/24  255.255.255.0    256 addresses, 254 hosts
/25  255.255.255.128  128 addresses, 126 hosts
/26  255.255.255.192   64 addresses,  62 hosts
/30  255.255.255.252    4 addresses,   2 hosts (point-to-point)
```

Private ranges: `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`; `127.0.0.0/8` is loopback. Public routing does not forward private addresses, so internal traffic needs NAT to reach the Internet.

## Subnetting and VLSM

To split a large subnet, borrow host bits for subnet bits. Splitting `192.168.1.0/24` into two `/25`s:

```
192.168.1.0/25    usable 192.168.1.1   – 192.168.1.126
192.168.1.128/25  usable 192.168.1.129 – 192.168.1.254
```

**VLSM** lets each subnet use a different length: point-to-point links use `/30` to save addresses, while server subnets use `/25` for headroom. Plan by asking "how many hosts per subnet", then choose the smallest prefix that fits.

## Configuration and verification

```
Huawei/H3C:
interface g0/0/1
 ip address 192.168.10.1 24
Cisco:
interface g0/0
 ip address 192.168.10.1 255.255.255.0
```

A host needs an address in the same subnet plus a gateway to communicate off-subnet; the gateway address must fall inside the host's subnet.

## Hands-on

Open Lab 2: configure Vlanif10 as 192.168.10.1/24 and Vlanif20 as 192.168.20.1/24, with the PCs pointing at them as gateways. Calculate both usable ranges on paper first, then configure and ping. Change the mask to /25 and observe which addresses stop communicating.
