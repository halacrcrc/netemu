---
title: "WAN Interconnect and Transport: SD-WAN, SRv6, and CloudWAN"
order: 41
level: IE
topic: mpls
lab: mpls-l3vpn
---

The WAN module is 33% of HCIE-Datacom V2.0. Branches no longer rely only on leased lines: **SD-WAN** orchestrates Internet/MPLS/5G underlays with an overlay, while carriers use **SRv6/CloudWAN** for programmable transport.

## The three layers of SD-WAN

- **Underlay**: physical links (MPLS, Internet, LTE/5G) providing reachability;
- **Overlay**: GRE/IPsec tunnels providing encryption and topology decoupling;
- **Controller**: centrally pushes policy and steers by application quality (A-FEC, link failover).

Key capabilities: zero-touch provisioning, application identification, on-demand path selection, centralized policy, and visual operations.

## SRv6 and CloudWAN

- **SRv6** encodes the path as an IPv6 segment list; the source decides which nodes to traverse;
- **BE (best effort)** and **Policy** modes; Policy enables explicit paths and traffic engineering;
- **Network slicing**: different services use different segment lists/resources without contention;
- **TI-LFA**: topology-independent loop-free backup with sub-second failover;
- **CloudWAN**: combines SRv6, EVPN, controllers, and cloud-network coordination into a transport solution.

## Relationship to MPLS L3VPN

MPLS L3VPN remains the mainstream enterprise leased-line model: PE uses VRF isolation, MP-BGP carries VPNv4, and labels forward traffic. Inter-AS options A/B/C apply. SRv6 can replace some LSP signaling and simplify transport; the two often coexist during evolution.

## Design concerns

- Do branches vary in number and link quality enough to need application-aware steering?
- How are IPsec/GRE keys and certificates managed?
- Do critical services need SRv6 Policy/slicing guarantees?
- Can the data plane keep forwarding if the controller fails?
- How does it interoperate with and migrate from existing MPLS VPN?

## Hands-on

Open Lab 8's MPLS L3VPN topology and understand the "VRF + two labels" leased-line model. Then imagine replacing the P network with SRv6 transport: the ingress PE pushes a segment list and intermediate nodes forward by SID. Drawing the label versus segment-list differences is the entry point to SD-WAN/CloudWAN evolution.
