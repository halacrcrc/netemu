---
title: "Campus Capstone: Multi-Protocol Design and Troubleshooting"
order: 15
level: IE
topic: campus
lab: campus-capstone
---

A real campus never tests one protocol in isolation. Access switches carve VLANs, aggregation switches terminate them, the core runs routing and egress NAT, and one mistake at any layer shows up as "some PC cannot ping". This lesson strings the earlier technologies into a **bottom-up troubleshooting path**.

## Reading the topology by layer

- **Access (SW1/SW2)**: access ports face hosts, trunks go upward, dual links form an LACP bundle;
- **Aggregation (DSW)**: inter-VLAN routing via SVIs, with the default route pointing at the core VRRP VIP;
- **Core (CORE1/CORE2)**: OSPF interconnect, VRRP redundancy, egress NAT;
- **Egress**: the public server sees only the NAT-translated address.

## Troubleshooting order: physical to application

1. **Links and ports**: `display interface brief` for Up/Down and VLAN membership;
2. **Layer 2**: `display vlan` and `display eth-trunk 1` to confirm trunk permissions and selected members;
3. **Layer 3**: `display ip routing-table` for the destination prefix, `display ospf peer` for neighbors;
4. **Gateway redundancy**: `display vrrp` for the Master and VIP;
5. **Egress**: `display nat session` to see whether translation happens;
6. **End to end**: ping from the PC to gateway, core, then Internet to narrow the fault hop by hop.

Common traps: a trunk that forgets to permit the VLAN, member ports configured as trunks while the logical interface is not (this simulator does not carry layer-2 config on the aggregate interface), a wrong OSPF wildcard, equal VRRP priorities making the Master unstable, and NAT configured inside-only without outside.

## Key configuration snippets

```
DSW:
vlan batch 10 20 99
interface vlanif 10
 ip address 192.168.10.1 24
ip route-static 0.0.0.0 0 10.0.0.1
CORE1:
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
interface g0/0
 vrrp 1 ip 10.0.0.1
 vrrp 1 priority 200
 vrrp 1 preempt
```

## Hands-on

Open Lab 9: complete VLANs, LACP, SVIs, VRRP, OSPF, and NAT. Verify PC1 → PC2 (same VLAN across buildings), PC1 → PC3 (inter-VLAN), and PC1 → 203.0.113.10 (NAT egress). If any step fails, walk the six layers from the bottom up — far faster than changing configuration on a hunch.
