---
title: "Static Routing and the Routing Table"
order: 24
level: IA
topic: routing
lab: campus-capstone
---

Every packet a router receives poses one question: which exit leads to the destination? The answer comes from the **routing table**. Static routes are entries you write by hand — simple, predictable, and the foundation for understanding dynamic routing.

## How the routing table decides

A route contains a destination prefix, a next hop or egress interface, an administrative distance (preference), and a metric. Forwarding compares them in order:

1. **Longest prefix match**: /32 beats /24, /24 beats /0;
2. If prefixes tie, compare **administrative distance / preference** (lower wins);
3. If those tie, compare **metric/cost**.

Defaults differ by vendor:

```
Cisco:        connected 0, static 1, eBGP 20, OSPF 110, iBGP 200
Huawei/H3C:   connected 0, static 60, OSPF 10, IS-IS 15, BGP 255
```

A static default route can therefore catch all unknown destinations; to let dynamic routes win instead, raise the static route's distance — a **floating static route**.

## Default and floating static routes

```
Huawei/H3C:
ip route-static 0.0.0.0 0 10.0.0.1
ip route-static 192.168.2.0 24 10.0.0.2 preference 200
Cisco:
ip route 0.0.0.0 0.0.0.0 10.0.0.1
ip route 192.168.2.0 255.255.255.0 10.0.0.2 200
```

The next hop must be **recursively reachable**: if 10.0.0.1 itself has no connected route or route, the static route never enters the table.

## Troubleshooting order

1. Check whether the destination prefix exists in `display ip routing-table` / `show ip route`;
2. Verify the next hop is reachable and the egress interface is up;
3. Check for a longer prefix or a lower distance overriding the route;
4. Check the return route — a failed ping often means "outbound exists, return path missing".

## Hands-on

Open Lab 9: DSW configures `ip route-static 0.0.0.0 0 10.0.0.1` toward the VRRP VIP, and the core adds return routes. Confirm the default route with `display ip routing-table`, then ping the gateway, core, and Internet hop by hop. Remove the default route and watch which traffic fails immediately.
