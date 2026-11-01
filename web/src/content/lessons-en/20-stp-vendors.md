---
title: "Spanning Tree, Three Names: PVST+ · MSTP · MSTP"
order: 20
level: MAP
topic: stp
lab: stp-redundancy
---

Layer-2 loops have no TTL, so broadcasts replicate forever. **Spanning tree** blocks redundant ports to cut a looped topology into a loop-free tree. All three vendors support standard STP/RSTP/MSTP, but Cisco adds the per-VLAN PVST+ family — the classic interoperability trap.

## What each vendor calls it

```
Cisco      PVST+ (one tree per VLAN, proprietary)
           Rapid-PVST+ (fast-converging variant)
           MSTP (802.1s, standard, interoperable with Huawei/H3C)
Huawei     STP / RSTP / MSTP
H3C        STP / RSTP / MSTP
```

- **PVST+**: each VLAN elects its own root and blocks independently, enabling per-VLAN load sharing at the cost of many BPDUs;
- **MSTP**: maps many VLANs to a few instances, one tree per instance, balancing load sharing against overhead;
- **Interoperability**: when Cisco meets Huawei/H3C, both sides should run MSTP with matching **region name, revision, and VLAN-to-instance mapping**, or each side computes its own tree.

## Real-world command reference

```
Cisco:
spanning-tree mode rapid-pvst
spanning-tree vlan 10 priority 4096
spanning-tree vlan 10 root primary

Huawei:
stp mode mstp
stp instance 1 priority 4096
stp root primary

H3C:
stp mode mstp
stp instance 1 priority 4096
stp root primary
```

Priority must be a multiple of 4096 and lower wins; `root primary` is a macro that picks a priority low enough to win the election.

## In this simulator

The simulator implements STP/RSTP/MSTP (MSTP computes per instance but does not negotiate region name/revision). The command table therefore focuses on basics supported everywhere: priority, port cost, disable, and show.

```
stp priority 4096
stp cost 100
display stp brief
```

## Hands-on

Open Lab 3: two switches form a loop over two links; observe the blocked port, raise SW1 to priority 4096 to become root, and watch the roles change. Shut down the primary link and PC1 should still reach PC2 after convergence. For per-VLAN load sharing, map different VLANs to different MSTP instances.
