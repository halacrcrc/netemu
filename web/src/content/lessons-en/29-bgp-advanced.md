---
title: "Advanced BGP: Path Selection and Route Reflection"
order: 29
level: IE
topic: bgp
lab: bgp-route-policy
---

BGP exchanges reachability plus path attributes. When the same prefix arrives from several neighbors, the router compares them in a fixed order; as the network grows, iBGP full mesh becomes unmanageable and needs route reflection.

## Path selection (simplified)

```
1. Weight (Cisco proprietary, higher wins)
2. LOCAL_PREF (higher wins; AS-wide egress policy)
3. Locally originated (network/aggregate) preferred
4. Shortest AS_PATH
5. Origin (IGP < EGP < Incomplete)
6. MED (lower wins; influences inbound traffic)
7. eBGP preferred over iBGP
8. Lowest IGP metric to NEXT_HOP
9. Lowest router ID / neighbor address (tie-breaker)
```

Mnemonic: **local preference first, then path length, then origin and MED, and finally the next hop**. In troubleshooting, identify which step actually decided the path.

## iBGP full mesh and route reflectors

iBGP has split horizon: a route learned from an iBGP neighbor is not passed to another iBGP neighbor. With N routers, the AS needs N(N−1)/2 sessions — unmanageable at scale. A **route reflector (RR)** may reflect routes to its clients, breaking split horizon; RRs and clients form a cluster, and the cluster ID prevents loops. A **confederation** splits one AS into sub-ASes to reduce session count.

## Common attributes and policy

- **LOCAL_PREF**: chooses the egress inside the AS, usually set by a route-policy on import;
- **MED**: suggests an ingress point to a neighbor, influencing how traffic enters;
- **Community**: tags routes (for example no-export, no-advertise) for bulk policy;
- **Next-Hop**: iBGP does not change it by default; ensure IGP reachability or use `next-hop-self`.

```
route-policy SET-LP permit node 10
 apply local-preference 200
bgp 65001
 peer 10.0.0.2 route-policy SET-LP import
```

## Stability

- **BFD** for fast neighbor failure detection;
- **Route dampening** so flapping routes do not repeatedly burn CPU;
- **Maximum prefixes** so a misbehaving neighbor cannot overwhelm the device;
- **GTSM/TTL security** against forged BGP packets.

## Hands-on

Open Lab 7 and watch R2 filter prefixes with a route-policy. Then consider: if R2 learns 192.168.1.0/24 from two neighbors, one with LOCAL_PREF 200 and one with a shorter AS_PATH, which wins? Write the answer in your notes and verify with `display bgp routing-table`.
