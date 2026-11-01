---
title: "OSPFv3: Link-State Routing for IPv6"
order: 44
level: IP
topic: ospfv3
lab: ospfv3
---

OSPFv3 is the IPv6 version of OSPF. Neighbors, areas, and SPF stay the same, but addressing, packets, and some mechanisms change: neighbors form over link-local addresses, Link-LSAs describe links, and the Router ID remains a 32-bit IPv4-format value.

## Key differences from OSPFv2

- **Per link, not per subnet**: OSPFv3 runs on interfaces/links, and one interface may carry several IPv6 prefixes;
- **Link-local addresses**: adjacencies usually form over `fe80::/10`, and next hops are link-local;
- **Link-LSA**: carries the interface's link-local address and prefixes, while SPF still uses Router-LSAs;
- **Authentication**: the OSPFv3 header no longer carries authentication; IPv6 IPsec is used instead;
- **Router ID**: still 32 bits, manually configured or taken from an IPv4 address.

## Configuration

```
Huawei/H3C:
ospfv3 1
 router-id 1.1.1.1
interface g0/0/1
 ipv6 address 2001:db8:12::1/64
 ospfv3 1 area 0
Cisco:
ipv6 router ospf 1
 router-id 1.1.1.1
interface g0/0
 ipv6 address 2001:db8:12::1/64
 ipv6 ospf 1 area 0
```

Verify with `display ospfv3 peer` / `show ipv6 ospf neighbor` and `display ipv6 routing-table` / `show ipv6 route`; OSPFv3 routes appear as `O`.

## Simulator simplifications

The simulator implements OSPFv3 **neighbor formation and prefix learning**: enabled interfaces periodically send their prefix lists, neighbors learn them, and IPv6 routes are installed. Full LSA flooding, DR election, and inter-area summarization are not modeled. That is enough for "directly connected routers learn each other's subnets"; complex topologies still need real devices.

## Hands-on

Open Lab 21: configure IPv6 addresses and OSPFv3 on R1/R2. `display ospfv3 peer` should show the peer Router ID, `display ipv6 routing-table` should show the remote subnet, and PC1 should ping6 PC2. Then remove `ospfv3 1 area 0` from R2 and watch the neighbor and routes disappear.
