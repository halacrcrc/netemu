---
title: "IP Multicast Basics: IGMP, PIM, and RPF"
order: 39
level: IP
topic: multicast
lab: campus-capstone
---

Unicast is one-to-one, broadcast is one-to-all, and **multicast** is one-to-many: the source sends one copy, the network replicates at forks, and only receivers that joined the group get it. Video conferencing, IPTV, and market data feeds depend on it.

## Addresses and membership

- Multicast addresses are `224.0.0.0/4`; `224.0.0.0/24` is link-local protocol space and `239.0.0.0/8` is private multicast;
- Hosts use **IGMP** to join and leave: v2 adds Leave, v3 supports source-specific (SSM);
- Routers maintain group membership to decide which interfaces need the stream.

## PIM: the multicast routing protocol

- **PIM-DM (dense mode)**: flood first, then prune; suits dense small networks;
- **PIM-SM (sparse mode)**: build a shared tree `(*,G)` rooted at the RP, then switch to the source tree `(S,G)` as traffic grows;
- **PIM-SSM**: specify source and group directly, no RP needed, ideal for live/video;
- **RPF check**: multicast packets must arrive on the interface on the shortest path back to the source, otherwise they are dropped — the core loop-prevention rule.

Multicast routing entries use `(S,G)` or `(*,G)` and are maintained separately from unicast routes.

## Typical troubleshooting

1. Did the receiver send an IGMP join?
2. Is the unicast route to the RP reachable?
3. Is the RPF interface correct (often changed by unicast routing)?
4. Does a multicast boundary/ACL block the group?
5. Is IGMP Snooping enabled at Layer 2 so multicast is not flooded like broadcast?

## In this simulator

The simulator focuses on unicast per-packet forwarding and does not implement IGMP/PIM; this lesson is conceptual and troubleshooting-oriented. Open Lab 9 and compare broadcast and unknown-unicast flooding to appreciate why multicast replicates at forks rather than flooding the whole network.
