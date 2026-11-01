---
title: "VXLAN and EVPN: Stretching Layer 2 Across Data Centers"
order: 12
level: IE
topic: vxlan
lab: evpn-dc
---

Live VM migration requires the same IP and the same gateway, which means both data centers must sit in **one layer-2 domain**. Traditional VLANs offer only 4094 IDs and cannot stretch across a layer-3 network. **VXLAN** solves this by wrapping the original Ethernet frame in a UDP packet and carrying it over a layer-3 underlay — giving layer 2 the reach of layer 3.

## VTEP, VNI, and encapsulation

- **VTEP**: the tunnel endpoint, usually a data-center leaf switch;
- **VNI**: a 24-bit network identifier; one VNI maps to one layer-2 domain (and usually to a local VLAN);
- **Encapsulation**: outer IP/UDP (destination port 4789) → VXLAN header (with VNI) → original Ethernet frame.

Broadcasts and unknown unicasts inside a VNI are replicated to every remote VTEP; known unicasts go straight through the matching tunnel. Because the outer header is ordinary IP, the underlay only needs layer-3 reachability.

```
DSW1:
vlan batch 10 100
interface vlanif 100
 ip address 10.0.0.1 24
vxlan local 10.0.0.1
vxlan vni 10000 vlan 10
vxlan peer 10.0.0.2
```

## From static peers to EVPN

Manual `vxlan peer` entries are fine for two switches, but at scale they become an N² configuration nightmare. **EVPN** uses BGP as the control plane, advertising "who I am and which VNIs I have" as routes (a simplified Type-3); peers learn them and add VTEPs automatically, after which the data plane can build tunnels.

```
DSW1:
evpn enable
evpn peer 10.0.0.2
```

The control plane handles discovery while the data plane remains VXLAN encapsulation — a clean separation that made EVPN the mainstream data-center design.

## Hands-on

Start with Lab 15: configure static peers and confirm H1 and H2, on the same subnet but in different DCs, can ping each other; a capture shows the outer 10.0.0.x header around the original Ethernet frame. Then open Lab 18: remove the static peers, use `evpn enable` plus `evpn peer`, wait about 2 seconds for advertisements, and `display evpn` / `display vxlan` should show the remote VTEP automatically while H1 still reaches H2.
