---
title: "Link Aggregation and Stacking: Many Links, Many Switches, One Logical Unit"
order: 35
level: IA
topic: lag
lab: lag-redundancy
---

Campus access has two recurring needs: **more bandwidth** and **device-level redundancy**. Link aggregation bundles physical links into one logical link; stacking turns several switches into one logical device. Both reduce STP blocking and add resilience.

## Link aggregation (LACP)

- Member ports join one aggregation group and negotiate with **LACP (802.3ad)**;
- Both ends `active`, or one `active` and one `passive`, can select members; two `passive` ends never send LACPDUs;
- Traffic is hashed by source/destination MAC, IP, and port, while a single flow stays on one link;
- STP sees the bundle as one logical port, so extra members are never blocked.

```
Cisco:
interface g0/0/1
 channel-group 1 mode active
Huawei:
interface g0/0/1
 eth-trunk 1
H3C:
interface g0/0/1
 port link-aggregation group 1
```

In this simulator, VLAN/trunk settings belong on the **member ports**; the aggregate interface itself does not carry layer-2 configuration.

## Stacking (iStack / IRF / StackWise)

- Switches share a **domain** and each gets a **member** ID;
- The highest priority becomes Master, and inter-member links become internal fabric that does not run STP;
- The stack presents one management IP and one configuration; if a member fails, the rest keep forwarding;
- The risk is **split-brain**: if all stack links fail, both members may think they are master, so real devices use MAD detection.

```
stack domain 10
stack member 1
stack priority 200
```

## Verification

```
display eth-trunk 1      show etherchannel summary
display stack            show stack
```

## Hands-on

Open Lab 5: add both inter-switch links to Eth-Trunk 1, confirm two members Up, and ping PC1 → PC2; shut down one member and service survives. Then open Lab 16: put both switches in one stack domain, see the Master and two members in `display stack`, and verify PC1 reaches across members.
