---
title: "VXLAN/EVPN in Three Data Center Vendors"
order: 21
level: MAP
topic: vxlan
lab: evpn-dc
---

VXLAN wraps layer-2 frames in UDP over a layer-3 network, and EVPN uses BGP as the control plane to discover VTEPs. The standards are shared, but Cisco Nexus, Huawei CloudEngine, and H3C data-center switches differ sharply in configuration model and commands.

## How each vendor organizes it

```
Cisco Nexus:
 feature vn-segment-vlan-based
 vlan 10
  vn-segment 10000
 interface nve1
  source-interface loopback0
  member vni 10000
   ingress-replication protocol bgp
 EVPN is advertised through address-family l2vpn evpn

Huawei CloudEngine:
 bridge-domain 100
  vxlan vni 10000
 interface Nve1
  source 10.0.0.1
  vni 10000 head-end peer-list 10.0.0.2
 EVPN is learned through BGP EVPN neighbors

H3C:
 vsi 100
  vxlan 10000
  tunnel 1
 interface Vsi-interface 100
  ...
 EVPN is advertised through the BGP EVPN address family
```

The pattern is three layers: **service instance (VLAN/BD/VSI) → VNI mapping → tunnel endpoint (NVE/VTEP)**. Cisco uses an `nve1` interface, Huawei uses `Nve1`, H3C uses `tunnel` plus `vsi`; the name differs, but the encapsulation is always VXLAN over UDP 4789.

## Control plane: static peers versus EVPN

- **Static peers**: manually list every remote VTEP, fine for a two-switch validation;
- **EVPN**: BGP advertises VTEPs and VNIs (simplified to Type-3 here), and peers learn them and add tunnels automatically;
- **Distributed anycast gateway**: real data centers also use EVPN Type-2/Type-5 routes for host routes and layer-3 gateways; this simulator covers layer-2 stretching only.

## In this simulator

The simulator uses one model: `vxlan local / vni / peer` for static tunnels and `evpn enable / evpn peer` for the control plane. All three dialects share the commands, keeping attention on the two core ideas — VNI mapping and peer discovery.

```
vxlan local 10.0.0.1
vxlan vni 10000 vlan 10
vxlan peer 10.0.0.2
evpn enable
evpn peer 10.0.0.2
```

## Hands-on

Start with Lab 15: static peers let H1 and H2 reach each other across DCs. Then open Lab 18: switch to EVPN, wait about 2 seconds for advertisements, and `display evpn` / `display vxlan` should show the remote VTEP automatically. Compare captures: outer 10.0.0.x plus UDP 4789, inner original Ethernet frame.
