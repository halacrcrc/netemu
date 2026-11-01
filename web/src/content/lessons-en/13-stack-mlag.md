---
title: "Stacking and M-LAG: Two Switches Acting as One"
order: 13
level: IE
topic: ha
lab: mlag-dc
---

When an access switch dual-homes to two aggregation switches, classic STP blocks one link — wasting half the bandwidth and waiting for convergence after a failure. **Stacking** and **M-LAG** attack the same problem from two directions: make two physical devices look like one to the outside.

## Stacking / IRF: genuinely one logical device

Several switches are configured with the same **domain**, each with a unique **member** ID, and interconnected by dedicated fabric links. The elected Master manages the whole stack, forwarding entries are synchronized, and the outside sees one management IP and one configuration.

- Member links are treated as an **internal bus**: STP does not run on them and they are never blocked;
- The highest Master priority wins; duplicate member IDs prevent a switch from joining;
- Upgrades and reboots can be rolled member by member without dropping service.

```
SW1:
stack domain 10
stack member 1
stack priority 200
SW2:
stack domain 10
stack member 2
stack priority 100
```

## M-LAG: aggregate logically, stay independent physically

M-LAG does not merge control planes: two DSWs run independently but share one **system MAC** and synchronize entries over a **peer-link**. The access switch dual-homes with LACP, and both links forward at once while the access switch sees a single LACP neighbor.

- The shared system MAC makes LACP believe the peer is one device;
- The peer-link carries cross-device traffic and synchronization and must be highly reliable;
- Broadcasts and unknown unicasts arriving over the peer-link are not sent back out the cross-device LAG (split horizon), preventing loops.

```
DSW1/DSW2:
mlag domain 1
interface g0/0/2
 mlag peer-link
```

## Hands-on

Open Lab 16: put both switches in the same stack domain and `display stack` should show the Master and two members while PC1 reaches PC2 across members. Then open Lab 17: configure `mlag domain 1` on both DSWs, dual-home SW1 with LACP, confirm the shared System MAC in `display mlag`, see two members Up in `display eth-trunk 1`, and verify PC1 reaches PC2. Shut down one uplink — service should not drop.
