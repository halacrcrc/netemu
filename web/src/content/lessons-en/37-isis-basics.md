---
title: "IS-IS Basics: NET, Levels, and DIS"
order: 37
level: IP
topic: isis
lab: campus-capstone
---

IS-IS is the second link-state protocol in the HCIP core syllabus. Like OSPF it computes paths with SPF, but its packets run directly over Layer 2, giving it strong extensibility; carriers and large data centers still rely on it.

## Three must-know concepts

- **NET**: Network Entity Title, for example `49.0001.0000.0000.0001.00`; area address + System ID + SEL, where the System ID uniquely identifies the router;
- **Level 1 / Level 2**: L1 handles the area interior, L2 forms the backbone; L1-2 routers belong to both, similar to an OSPF ABR;
- **DIS**: the Designated Intermediate System on a broadcast segment, similar to a DR but with **no backup DIS** and preemption by a higher-priority router.

IS-IS describes topology with **LSPs** and synchronizes databases with CSNP/PSNP; its TLV structure naturally carries IPv6, SR, and TE extensions.

## Configuration

```
Huawei/H3C:
isis 1
 network-entity 49.0001.0000.0000.0001.00
interface g0/0/1
 isis enable
Cisco:
router isis
 net 49.0001.0000.0000.0001.00
interface g0/0
 ip router isis
```

Note that Cisco uses `ip router isis` on the interface, while Huawei/H3C use `isis enable`.

## Compared with OSPF

- **Area boundary**: OSPF divides areas per interface; IS-IS divides L1/L2 per router;
- **Designated device**: OSPF has DR/BDR; IS-IS has only a DIS, with no backup;
- **Encapsulation**: OSPF runs over IP (protocol 89); IS-IS runs directly over Layer 2;
- **Extensibility**: IS-IS TLVs are flexible and commonly carry SR/SRv6.

Verify with `display isis peer` / `show isis neighbors` and `display isis lsdb` / `show isis database`.

## Hands-on

In Lab 9's core links, apply the OSPF reasoning: configure the same area address and different System IDs on both cores, enable IS-IS on the interfaces, and observe L1 neighbors and routes. Then consider: what happens if one router is L2-only and the other is L1-2?
