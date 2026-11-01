---
title: "IS-IS and Segment Routing"
order: 30
level: IE
topic: isis
lab: campus-capstone
---

IS-IS is a link-state protocol common in carrier and large data-center networks. It resembles OSPF but runs directly over Layer 2; **Segment Routing (SR)** encodes the path in labels or IPv6 addresses so the forwarding plane no longer needs per-hop protocol state.

## Three IS-IS keywords

- **NET**: Network Entity Title, for example `49.0001.0000.0000.0001.00`; the System ID uniquely identifies the router;
- **Level 1 / Level 2**: L1 inside an area, L2 the backbone; an L1-2 router belongs to both, much like an OSPF ABR;
- **DIS**: Designated Intermediate System on a broadcast segment, similar to a DR but with **no backup DIS** and a default 10-second election cycle.

IS-IS packets are encapsulated directly in Layer 2, not IP, and its TLV extensibility carries IPv6, SR, and TE capabilities. It describes topology with **LSPs** and synchronizes databases with CSNP/PSNP.

## The core of Segment Routing

Traditional MPLS builds LSPs hop by hop with LDP/RSVP; SR lets the **source** encode the path as a label stack (SR-MPLS) or an IPv6 segment list (SRv6):

- **Node SID / Prefix SID**: a segment identifier per router or prefix;
- **Adjacency SID**: identifies a specific adjacency;
- **SRGB**: the global label block that keeps labels unique in the domain;
- **TI-LFA**: topology-independent loop-free backup with sub-second failover.

```
SR-MPLS forwarding:
Ingress pushes [Node-SID(R2), Node-SID(R4)]
R2 pops its own SID and forwards by the top label
R4 receives, pops, and delivers to the destination prefix
```

SRv6 writes the SID as an IPv6 address (Locator + Function), removing the need for MPLS labels and enabling SDN programming and service chaining.

## Configuration and verification

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

Verify with `display isis peer` / `show isis neighbors` and `display isis lsdb` / `show isis database`.

## Hands-on

In Lab 9's core links, reason through replacing OSPF with IS-IS: two core routers form an L1 adjacency in the same area and then redistribute external routes. Real SR deployment also needs a controller and label planning; this simulator covers only IS-IS neighbors and route learning.
