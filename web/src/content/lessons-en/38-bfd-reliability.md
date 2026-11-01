---
title: "BFD and Network Reliability: Millisecond Failure Detection"
order: 38
level: IP
topic: fhrp
lab: campus-capstone
---

Routing protocols detect neighbors with Hellos: OSPF's dead interval defaults to 40 seconds and BGP is even slower. After a link failure, service waits tens of seconds to switch. **BFD** uses a lightweight heartbeat to cut detection to milliseconds and triggers fast failover in routing and gateway protocols.

## How BFD works

- Both ends establish a BFD session and send probes periodically (down to millisecond scale);
- After a configured number of missed probes the session goes Down and notifies associated protocols;
- **Asynchronous mode** sends probes periodically; **echo mode** loops probes back to reduce peer processing;
- It commonly binds to **OSPF/BGP/VRRP/static routes**: when BFD goes Down, the protocol converges or switches immediately.

## Four layers of reliability

1. **Link**: link aggregation (LACP), dual uplinks;
2. **Device**: stacking/IRF, M-LAG/vPC/DRNI;
3. **Gateway**: VRRP/HSRP active-standby;
4. **Path**: fast IGP convergence, BGP multipath, FRR/TI-LFA.

BFD does not replace these; it lets them **learn about failures faster**. For example, VRRP defaults to 3.5-second failover, but with BFD it can drop to hundreds of milliseconds.

## Verification approach

```
display bfd session all        show bfd neighbors
display vrrp                  show vrrp
display ospf peer             show ip ospf neighbor
```

Troubleshoot by confirming the BFD session is Up, checking whether the associated protocol received the Down notification, and finally verifying the backup path actually works.

## In this simulator

The simulator implements VRRP and link aggregation but does not model BFD packets. Open Lab 9 to observe VRRP failover timing, then consider how much faster BFD would make it and which scenarios (such as a flapping single link) should not use an overly aggressive BFD timer.
