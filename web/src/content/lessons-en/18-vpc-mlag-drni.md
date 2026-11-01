---
title: "Cross-Device Aggregation: vPC · M-LAG · DRNI"
order: 18
level: MAP
topic: mlag
lab: mlag-dc
---

When an access switch dual-homes to two aggregation switches, classic STP blocks one uplink. **Cross-device link aggregation** makes the two aggregation switches look like one LACP neighbor: both uplinks forward at once, and a failure heals without waiting for STP. Cisco, Huawei, and H3C give it different names, but the problem and the solution are the same.

## What each vendor calls it

```
Cisco      vPC (virtual Port Channel, Nexus data center)
Huawei     M-LAG (Multi-Chassis Link Aggregation, CloudEngine)
H3C        DRNI (Distributed Resilient Network Interconnect)
```

Four ideas capture the design:

- **Two control planes**: each device runs its own protocols and they do not merge into one device (unlike stacking);
- **Shared system identity**: the access switch's LACP sees one System MAC / system ID;
- **Peer-link**: the interconnect that carries cross-device traffic and synchronizes entries;
- **Split horizon**: broadcasts and unknown unicasts received over the peer-link are not sent back out the cross-device LAG, preventing loops.

## Real-world command reference

Commands vary by version; the skeletons below show the model (consult each vendor's documentation for exact syntax).

```
Cisco NX-OS (vPC):
vpc domain 1
 peer-keepalive destination 10.0.0.2
interface port-channel 10
 vpc peer-link
interface port-channel 20
 vpc 20

Huawei (M-LAG):
dfs-group 1
 peer-link 1
interface Eth-Trunk 10
 peer-link 1
interface Eth-Trunk 20
 mlag 10

H3C (DRNI):
drni system-mac 0000-0000-0001
drni system-number 1
drni system-priority 100
interface Bridge-Aggregation 10
 drni ip-portal
interface Bridge-Aggregation 20
 drni dr-group 1
```

Terminology mapping: vPC **peer-keepalive** ≈ M-LAG **DFS group** ≈ DRNI **IPP/DRCP**; vPC **member vPC** ≈ M-LAG **mlag id** ≈ DRNI **DR group**.

## How it differs from stacking

- **Control plane**: stacking merges into one device; cross-device aggregation keeps two independent devices;
- **Configuration**: stacking has one copy; cross-device aggregation has two copies that must stay consistent;
- **Failure domain**: a stack link failure needs MAD against dual-active; a peer-link failure carries a similar dual-active risk;
- **Upgrade**: stacking upgrades as a whole or in a rolling sequence; cross-device aggregation can upgrade one device at a time with less impact.

## In this simulator

The simulator models it with `mlag domain`: two switches in the same domain share system MAC `02:00:00:<domain>:01/02`; an interface running `mlag peer-link` becomes the peer-link; LACP dual-homing from the access switch selects both links into one bundle.

```
DSW1/DSW2:
mlag domain 1
interface g0/0/2
 mlag peer-link
```

## Hands-on

Open Lab 17: configure `mlag domain 1` on both DSWs and dual-home SW1 with LACP. `display mlag` should show the shared System MAC, `display eth-trunk 1` two members Up, and PC1 should reach PC2. Shut down one uplink to confirm service survives, then reason about what a real network does when the peer-link fails.
