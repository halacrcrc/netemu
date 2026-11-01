---
title: "Inter-AS MPLS VPN and Traffic Engineering"
order: 31
level: IE
topic: mpls
lab: mpls-l3vpn
---

Single-domain L3VPN means "PE pushes two labels, P swaps the outer label, and the egress PE delivers by VPN label". Real carrier networks span multiple ASes and must also choose paths by bandwidth and latency — that is what inter-AS VPN and MPLS TE solve.

## Three inter-AS L3VPN options

- **Option A (back-to-back VRFs)**: ASBRs connect through VRF subinterfaces and rebuild the VPN hop by hop; simple but poorly scalable;
- **Option B (MP-eBGP)**: ASBRs exchange VPNv4 routes over MP-BGP and swap labels; a common middle ground;
- **Option C (multi-hop MP-eBGP)**: PEs exchange VPNv4 directly while ASBRs only forward labeled packets; most scalable and most complex.

The key differences are **where VPN routes are exchanged** and **which layer reallocates labels**.

## RD and RT

- **RD (Route Distinguisher)**: 8 bytes prepended to an IPv4 prefix so identical subnets in different VRFs stay unique in MP-BGP;
- **RT (Route Target)**: an extended community deciding which VRFs export or import a route;
- RD provides uniqueness, RT provides import/export policy — do not confuse them.

## MPLS TE: constraint-based path selection

IGP picks the shortest path by metric and ignores bandwidth. **TE** first computes a path that satisfies bandwidth and affinity constraints (CSPF), then signals a bandwidth-reserved LSP with **RSVP-TE**:

- **Explicit path**: manually specify the nodes or links to traverse;
- **FRR**: pre-build backup LSPs for fast failover;
- **Affinity**: avoid or prefer certain links, such as high-latency ones.

**SR-TE** expresses the path as a segment list instead of hop-by-hop RSVP signaling, making it the mainstream approach in new transport networks.

## Verification and troubleshooting

```
display mpls lsp            show mpls lsp
display bgp vpnv4 all routing-table
display ip routing-table vpn-instance CUST1
```

Troubleshoot in order: VRF interface binding → RD/RT import-export → MP-BGP neighbor → LSP label stack → return LSP. In inter-AS scenarios, confirm that the **return labels** are allocated correctly on the other side.

## Hands-on

Open Lab 8: first get the single-domain static LSP working, then imagine the P router as two ASBRs and reason through Option B's label changes: ingress pushes transport + VPN labels, the ASBR swaps the transport label while keeping the VPN label, and the egress PE delivers by VPN label. Drawing each label stack on paper beats memorizing commands.
