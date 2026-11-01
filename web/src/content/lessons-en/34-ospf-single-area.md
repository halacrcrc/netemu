---
title: "Single-Area OSPF: Neighbors, DR, and Shortest Paths"
order: 34
level: IA
topic: ospf
lab: campus-capstone
---

In the HCIA IP networking module, **single-area OSPF** is the first dynamic routing protocol after static routes. Routers discover neighbors, exchange link-state information, and run SPF to compute loop-free shortest paths.

## How neighbors form

- Routers send **Hello** packets periodically (10s default, dead interval 40s);
- Hello carries Router ID, Area ID, Hello/Dead timers, authentication, and mask;
- Mismatched parameters prevent adjacency; states progress Down → Init → 2-Way → ExStart → Exchange → Loading → **Full**;
- On broadcast segments a **DR/BDR** is elected: highest priority wins, ties broken by Router ID; the DR collects and floods LSAs, reducing adjacencies.

OSPF uses multicast `224.0.0.5` (AllSPFRouters) and `224.0.0.6` (AllDRouters).

## Single-area configuration

Put every interface in area 0 and advertise subnets with wildcard masks:

```
Huawei/H3C:
ospf 1
 area 0
  network 10.1.0.0 0.0.0.3
Cisco:
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
```

- **Wildcard**: 0 means must match, 1 means ignore; `0.0.0.3` matches a /30;
- **Cost** defaults to interface bandwidth and can be set manually with `ospf cost`;
- Advertise only the subnets that should participate, keeping management and Internet interfaces out.

## Verification and common issues

```
display ospf peer          show ip ospf neighbor
display ip routing-table   show ip route
```

Stuck in Init/2-Way usually means mismatched Hello parameters or DR election issues. Full neighbors with no routes point to wrong advertisements/wildcards, or a longer prefix/lower distance overriding the route.

## Hands-on

Open Lab 9: CORE1 and CORE2 form neighbors in area 0 and learn 10.1.0.0/30 plus business subnets. Check the Full state with `display ospf peer`, then the `O` routes; break one wildcard mask and watch the neighbor and routes react.
