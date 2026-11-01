---
title: "VRF and MPLS L3VPN: Many Virtual Networks on One Physical Network"
order: 7
level: IP
topic: mpls
lab: mpls-l3vpn
---

Two customers rent the same provider backbone but demand "we cannot see each other, and our addresses may overlap". The answer has two layers: **VRF** isolates routing tables and interfaces inside a device, while **MPLS** forwards labeled packets across the backbone so P routers never need to inspect the customer IP header.

## VRF: multiple routing tables in one router

A normal router has one global routing table. Once a VRF is created and an interface is bound to it, that interface's connected and static routes go into a **VRF-specific table**; different VRFs can each use 192.168.1.0/24 without conflict. To interconnect VRFs you must explicitly import routes — this simulator does it through the `vrf` argument of a static LSP.

```
Huawei/H3C:
ip vpn-instance CUST1
interface g0/0/0
 ip binding vpn-instance CUST1
 ip address 192.168.1.1 24
Cisco:
vrf definition CUST1
interface g0/0/0
 ip vrf forwarding CUST1
 ip address 192.168.1.1 255.255.255.0
```

## MPLS: how the label stack moves

The ingress PE pushes **two labels**: an outer transport label that lets P routers forward hop by hop, and an inner VPN label that tells the egress PE which VRF to deliver to. P routers only look at and swap the outer label; the egress PE pops it and uses the inner label to hand the packet to the right VRF — that is why label switching is faster than hop-by-hop IP lookup.

This simulator models the path with **static LSPs**:

```
PE1:
static-lsp ingress to-ce2 destination 192.168.2.0 24 \
  nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001
P:
static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200
PE2:
static-lsp egress vpn1001 incoming-label 1001 vrf CUST1
```

The Cisco form prefixes the command with `mpls`: `mpls static-lsp ingress ...`. The return path needs its own LSP — MPLS does not build the reverse path for you.

## Hands-on

Open Lab 8: complete the VRF and forward/return LSPs on PE1, P, and PE2. `display mpls lsp` / `show mpls lsp` should show each LSP's VRF and VPN label; CE1 and CE2 should ping each other, and a capture shows the outer label being swapped at every hop.
