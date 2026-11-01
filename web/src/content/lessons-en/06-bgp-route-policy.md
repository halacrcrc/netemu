---
title: "BGP and route-policy: Inter-Domain Routing with Policy"
order: 6
level: IP
topic: bgp
lab: bgp-route-policy
---

OSPF answers "how do I get there inside my autonomous system"; **BGP** answers "how do I get there across autonomous systems (ASes)". BGP ignores link bandwidth and exchanges **path vectors** — which ASes a route has traversed — and picks paths using attributes such as AS_PATH and LOCAL_PREF.

## The minimal eBGP loop

Two routers in different ASes become neighbors and advertise their own subnets:

```
Huawei/H3C:
bgp 65001
 peer 10.0.0.2 as-number 65002
 network 192.168.1.0 255.255.255.0
Cisco:
router bgp 65001
 neighbor 10.0.0.2 remote-as 65002
 network 192.168.1.0 mask 255.255.255.0
```

Note that `network` only *permits* advertisement; the prefix must actually exist in the local routing table (connected or static) before BGP will send it.

## route-policy: a gate for routes

A policy is a series of **nodes** evaluated in ascending order: if `if-match` hits, the `apply` actions run and evaluation stops; otherwise the next node is tried. **Falling off the end means implicit deny**, so filter policies usually end with an empty permit node.

```
Huawei/H3C:
acl 2000
 rule permit source 172.16.9.0 0.0.0.255
route-policy FILTER deny node 10
 if-match acl 2000
route-policy FILTER permit node 20
bgp 65002
 peer 10.0.0.1 route-policy FILTER import
Cisco:
access-list 2000 permit 172.16.9.0 0.0.0.255
route-map FILTER deny 10
 match ip address 2000
route-map FILTER permit 20
router bgp 65002
 neighbor 10.0.0.1 route-map FILTER in
```

`import` (Cisco `in`) filters routes **as they are received**, while `export` (Cisco `out`) filters them **as they are sent** — reversing the direction is the most common lab mistake.

## Hands-on

Open Lab 7: R2 uses an ACL to match 172.16.9.0/24 exactly, then a route-policy denies it on import. `display bgp routing-table` / `show ip bgp` should show only 192.168.1.0/24; PC2 fails to ping the controlled server but still reaches PC1 — the policy only affects the route it should.
