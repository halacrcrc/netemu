---
title: "IPv6 Basics: Addresses, NDP, and Dual Stack"
order: 25
level: IA
topic: ipv6
lab: office-l3
---

IPv6 is not just "a longer IPv4". It redesigns addressing, discovery, and the header together: 128-bit addresses, no broadcast, NDP instead of ARP, and addresses that can configure themselves.

## What the addresses look like

128 bits are written as eight hexadecimal groups, with `::` compressing consecutive zeros (only once):

```
2001:0db8:0000:0000:0000:0000:0000:0001  →  2001:db8::1
```

Common types:

- **Global unicast** `2000::/3`: routable on the public Internet;
- **Link-local** `fe80::/10`: auto-generated per interface, valid only on the link, often used as the next hop for routing protocols;
- **Unique local** `fc00::/7`: similar to private addresses;
- **Multicast** `ff00::/8`: there is no broadcast; multicast replaces it;
- **Loopback** `::1`.

An interface usually has both a link-local and a global address; `/64` is the standard subnet size because it enables SLAAC.

## NDP: the ARP replacement

Neighbor Discovery runs over ICMPv6:

- **NS/NA**: resolve a neighbor's MAC (like ARP);
- **RS/RA**: routers advertise prefixes and the default gateway, letting hosts **SLAAC** an address;
- **Redirect**: tell a host about a better next hop.

With no broadcast, NDP queries a **solicited-node multicast** address (`ff02::1:ffXX:XXXX`), disturbing only the relevant hosts.

## Configuration and verification

```
Huawei/H3C:
interface g0/0/1
 ipv6 address 2001:db8:10::1/64
ipv6 route-static 2001:db8:20:: 64 2001:db8:10::2
Cisco:
interface g0/0
 ipv6 address 2001:db8:10::1/64
ipv6 route 2001:db8:20::/64 2001:db8:10::2
```

Verify with `show ipv6 interface brief` / `display ipv6 interface brief`, `show ipv6 route` / `display ipv6 routing-table`, and `ping ipv6 2001:db8:20::2`.

## IPv6 routing and OSPFv3

IPv6 static routes and OSPFv3 are common HCIP topics:

```
ipv6 route 2001:db8:20::/64 2001:db8:10::2
ospfv3 1
 router-id 1.1.1.1
interface g0/0
 ospfv3 1 area 0
```

OSPFv3 keeps the same neighbor and area concepts as OSPFv2, but forms neighbors over link-local addresses and uses Link-LSAs to carry link information; the Router ID remains a 32-bit IPv4-format value. Troubleshoot by checking `display ospfv3 peer` for Full, then the IPv6 routing table.

## Transition: dual stack first

The safest transition is **dual stack**: an interface carries both IPv4 and IPv6, and applications choose based on DNS. NAT64/DNS64, 6PE/6VPE, and tunnels also exist, but dual stack is the foundation.

## Hands-on

Open Lab 2's topology, add IPv6 addresses to SW1's VLAN interfaces and the two PCs (same /64), and verify reachability with `ping6`. Then intentionally misconfigure a prefix length and watch neighbor discovery and the routing table change.
