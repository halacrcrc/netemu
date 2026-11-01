---
title: "First-Hop Redundancy, Three Names: HSRP · VRRP · VRRP"
order: 19
level: MAP
topic: fhrp
lab: campus-capstone
---

A host has exactly one default gateway. If that gateway dies, the host is cut off. **First-hop redundancy protocols (FHRP)** let two routers share one virtual gateway IP: the active device forwards and answers ARP, the standby listens, and takes over when the active one disappears.

## What each vendor calls it

```
Cisco       HSRP (Hot Standby Router Protocol, proprietary)
            VRRP (standard, also supported by Cisco)
            GLBP (proprietary, gateway load balancing)
Huawei      VRRP (standard)
H3C         VRRP (standard)
```

HSRP dominates Cisco environments, while Huawei and H3C use **VRRP**. The goal is identical, but the details differ:

```
Item              HSRP                         VRRP
Roles             Active / Standby             Master / Backup
Virtual MAC       0000.0c07.acXX               0000.5e00.01XX
Default priority  100                          100
Preempt default   disabled                     enabled
Standard          Cisco proprietary            RFC 5798
```

## Real-world command reference

```
Cisco HSRP:
interface g0/0
 standby 1 ip 10.0.0.1
 standby 1 priority 200
 standby 1 preempt

Huawei/H3C VRRP:
interface g0/0/0
 vrrp vrid 1 virtual-ip 10.0.0.1
 vrrp vrid 1 priority 200
 vrrp vrid 1 preempt-mode
```

Note that HSRP does **not** preempt by default: a recovered master will not take the role back unless `preempt` is configured. VRRP preempts by default, so a recovered master becomes Master again.

## In this simulator

The simulator implements standard **VRRP**: the higher priority becomes Master and a tie is broken by interface IP; the Master answers ARP for the virtual IP; advertisements run every 1s and failover after 3.5s. HSRP is not modeled separately, but the same active/standby logic applies.

```
vrrp 1 ip 10.0.0.1                (Cisco)
vrrp vrid 1 virtual-ip 10.0.0.1   (Huawei/H3C)
```

Verify with `show vrrp` / `display vrrp` to see the Master, priority, and virtual IP.

## Hands-on

Open Lab 9: CORE1 and CORE2 share VRRP VIP 10.0.0.1, with CORE1 at priority 200 as Master. Shut down CORE1's uplink and watch CORE2 take over while the host gateway IP stays the same. Then consider: with equal priorities, who becomes Master?
