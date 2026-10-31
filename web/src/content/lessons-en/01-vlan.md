---
title: "VLAN and Trunk: Isolating Broadcast Domains"
order: 1
topic: vlan
lab: office-l2
---

By default, every port on a switch sits in the same broadcast domain — one host sends an ARP broadcast and the whole network hears it. **VLANs logically slice one physical switch into several**, with each VLAN acting as its own broadcast domain.

## Two core concepts

- **Access port**: Connects to hosts, belongs to exactly one VLAN, and sends/receives frames without tags.
- **Trunk port**: Connects switches, carries multiple VLANs at once, and tags frames with 802.1Q headers (except on the native VLAN).

## Why trunks are needed

When the finance department's hosts (VLAN 10) are spread across two switches, VLAN 10 traffic must cross between those switches — over a single cable. A trunk adds a 4-byte 802.1Q tag to each frame so the two ends can tell VLANs apart.

The critical difference: **Cisco trunks permit all VLANs by default, while Huawei/H3C trunks permit only VLAN 1** — you must allow VLANs explicitly:

```
Huawei/H3C:
 port link-type trunk
 port trunk allow-pass vlan 10
Cisco:
 switchport mode trunk
 switchport trunk allowed vlan 10
```

## How VLANs talk to each other

Different VLANs cannot communicate even when they share the same subnet (Layer 2 isolation). Reaching across them requires a Layer 3 device:

- On a Layer 3 switch, give each VLAN an **SVI** (called Vlanif on Huawei) to serve as its gateway;
- Or use **router-on-a-stick** on a router (subinterfaces plus 802.1Q).

Lab 2 in this simulator demonstrates the SVI approach: after traffic crosses VLANs, the TTL of the reply drops by 1 — direct evidence that it went through one Layer 3 forwarding hop.

## Hands-on

Open Lab 1, "Layer 2 Office Network (VLAN Isolation)": create VLANs, configure trunks, then use ping to confirm that hosts in the same VLAN reach each other while cross-VLAN traffic is blocked; select a link and enable packet capture to watch the 802.1Q tags on the trunk.
