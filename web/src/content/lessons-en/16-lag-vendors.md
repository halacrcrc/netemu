---
title: "Link Aggregation, Three Names: EtherChannel / Eth-Trunk / Bridge-Aggregation"
order: 16
level: MAP
topic: lag
lab: lag-redundancy
---

If you cable two switches with several links and do nothing else, STP keeps one and blocks the rest. **Link aggregation** bundles the physical links into one logical link: added bandwidth, mutual backup, and a single port from STP's point of view.

## What each vendor calls it

```
Feature            Cisco                      Huawei                    H3C
Aggregation        EtherChannel               Eth-Trunk                 Bridge-Aggregation
Member join        channel-group 1 mode ...   eth-trunk 1               port link-aggregation group 1
LACP modes         active / passive           lacp-static / manual      dynamic / static
Show               show etherchannel summary  display eth-trunk 1       display link-aggregation verbose
```

Cisco also has the proprietary **PAgP**, while Huawei and H3C support standard **LACP (802.3ad)** or manual mode. For multi-vendor interconnects, use LACP: `active` against `passive`, or `active` on both ends. Two `passive` ends never send LACPDUs and the bundle never comes up.

## Key ideas

- **Active/standby members**: LACP selects which members may forward; the rest stay ready and take over on failure;
- **Load sharing**: hashing over MAC/IP/port spreads flows across members, while one flow stays on one link;
- **STP view**: the bundle is a single logical port, so extra members are never blocked;
- **Consistency**: member speed, duplex, and VLAN settings should match, or only some members may be selected.

## In this simulator

The simulator uses one LACP model: members join a group, exchange LACPDUs, and forward only when both sides are `Selected`; the logical port participates in STP. Note that VLAN/trunk configuration belongs on the **member ports** here — the aggregate interface itself does not carry layer-2 settings.

```
Cisco:
interface g0/0/1
 channel-group 1 mode active
Huawei/H3C:
interface g0/0/1
 eth-trunk 1
H3C:
interface g0/0/1
 port link-aggregation group 1
```

## Hands-on

Open Lab 5: add both inter-switch links to Eth-Trunk 1, confirm two members Up in `display eth-trunk 1`, and ping PC1 → PC2. Shut down one member and the service should survive — the value of aggregation over STP.
