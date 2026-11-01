---
title: "OSPF and VRRP: Dynamic Routing and Gateway Redundancy"
order: 5
level: IP
topic: ospf
lab: campus-capstone
---

Static routes are fine for a small network, but every link change means editing them by hand. **OSPF** lets routers discover each other and exchange link-state information so they can compute loop-free shortest paths automatically. **VRRP** lets two gateways share one virtual IP, with the backup taking over when the master fails — clients never notice.

## How OSPF converges

- Each router sends **Hello** packets (every 10 seconds by default) to find neighbors;
- Neighbors exchange **LSAs** (link-state advertisements) and build a topology database;
- The SPF (Dijkstra) algorithm computes the shortest path to every subnet and installs it in the routing table;
- On a topology change, only the affected part is recomputed, which converges far faster than waiting for a route to age out.

In multi-area designs, **area 0 is the backbone**, and every other area must connect to it. The wildcard mask used in the `network` statement works like an ACL wildcard — it is inverted: `0.0.0.3` matches a /30.

```
Huawei/H3C:
ospf 1
 area 0
  network 10.1.0.0 0.0.0.3
Cisco:
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
```

## VRRP: a gateway IP that never goes down

Each router keeps its own real IP and both share a **virtual IP (VIP)**. The higher priority becomes Master and answers ARP and forwards traffic; the Backup listens for advertisements and takes over when they stop. With **preempt** enabled, the original Master reclaims the role when it recovers.

```
Huawei/H3C:
vrrp vrid 1 virtual-ip 10.0.0.1
vrrp vrid 1 priority 200
vrrp vrid 1 preempt-mode
Cisco:
vrrp 1 ip 10.0.0.1
vrrp 1 priority 200
vrrp 1 preempt
```

Verification: `display ospf peer` / `show ip ospf neighbor` shows whether neighbors reach FULL; `display vrrp` / `show vrrp` shows the Master and priority.

## Hands-on

Open Lab 9, the campus capstone: CORE1 and CORE2 run OSPF for the interconnect and share 10.0.0.1 as the DSW default gateway via VRRP. Shut down CORE1's uplink and watch the VRRP switchover and OSPF reconvergence restore service.
