---
title: "Advanced Multicast: PIM-SM, SSM, and RPF"
order: 32
level: IE
topic: multicast
lab: campus-capstone
---

Multicast lets a source send one copy while the network replicates at forks, and only receivers that joined the group get it. IP multicast basics are in Lesson 39; this lesson focuses on **routing protocols, the RP, and loop prevention** in enterprise and carrier multicast.

## Three PIM modes

- **PIM-DM (dense mode)**: flood first, then prune; suits dense small networks;
- **PIM-SM (sparse mode)**: build a shared tree `(*,G)` rooted at the RP, then switch to the source tree `(S,G)` as traffic grows;
- **PIM-SSM**: receivers specify `(S,G)` directly, no RP needed, ideal for live/video.

The key to PIM-SM is the **RP (rendezvous point)**: sources register with the RP, receivers join toward it, and after the shared tree is built the last-hop router may perform an **SPT switchover** to the source tree for lower latency.

## RPF: the core loop-prevention check

Multicast forwarding does not look at the destination; it checks the **source**:

- Packets must arrive on the interface on the shortest path back to the source (the RPF interface);
- Packets arriving on any other interface are dropped;
- Unicast routing changes alter the RPF interface, so multicast failures are often "the unicast route changed".

Multicast routing entries use `(S,G)` or `(*,G)` and are maintained separately from unicast routes. A practical troubleshooting order: IGMP join → RP reachability → RPF interface → multicast boundary/ACL → Layer-2 IGMP Snooping.

## Layer 2 and QoS

- **IGMP Snooping**: switches listen to IGMP and forward multicast only to ports with members, avoiding broadcast-like flooding;
- **Multicast QoS**: live streams can be marked with DSCP (such as AF41) and placed in an LLQ/priority queue at congestion points; QoS basics are in Lesson 46.

## In this simulator

The simulator focuses on unicast per-packet forwarding and does not implement IGMP/PIM; this lesson is protocol- and troubleshooting-oriented. Open Lab 9 and compare broadcast and unknown-unicast flooding to understand why multicast replicates at forks instead of flooding the whole network.

## Hands-on

Open Lab 9 and draw the "source—RP—receiver" shared tree and the "source—receiver" source tree, marking each router's RPF interface. Then suppose the unicast route moves the RPF interface to the other side and reason where multicast traffic will be dropped.
