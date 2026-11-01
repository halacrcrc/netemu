---
title: "Isolation Inside One VLAN: MUX VLAN · Private VLAN · Port Isolation"
order: 43
level: MAP
topic: isolation
lab: port-isolation
---

One VLAN may contain devices that must communicate and devices that must never talk to each other. Creating another VLAN would multiply subnets and gateways, so vendors offer **Layer-2 isolation inside the same broadcast domain**. Cisco, Huawei, and H3C name it differently but solve the same problem.

## What each vendor calls it

```
Cisco      Private VLAN (PVLAN)
           primary / isolated / community VLAN
           promiscuous / isolated / community ports
Huawei     MUX VLAN
           principal / separate / group VLAN
           principal / separate / group ports
H3C        port isolation (port-isolate)
           PVLAN/MUX VLAN solutions also exist
```

Typical behavior:

- **principal / promiscuous**: reaches every port, usually the gateway or a server;
- **separate / isolated**: cannot reach each other, only the principal;
- **group / community**: members reach each other, not other groups, and can reach the principal.

## The simulator model

The simulator unifies both into port roles and isolation groups, checked during Layer-2 forwarding:

```
mux-vlan principal          # reaches every port
mux-vlan separate           # isolated from other separate and all group ports
mux-vlan group 1            # same group communicates, cross-group isolated
port-isolate enable group 1 # same isolation group cannot communicate
switchport protected        # Cisco protected: all protected ports isolated
```

Inspect with `display mux-vlan` / `display port-isolate`; Cisco uses `show mux-vlan` / `show port-isolate`.

## Common designs

- Hotels/dorms: one separate port per room, only the principal gateway is reachable;
- Guest campus: isolated ports reach only the gateway;
- Server zone: community groups isolate tenants while allowing intra-group traffic;
- Port isolation: bulk isolation at access, simpler than PVLAN.

Remember that isolation is **Layer 2**. If Layer-3 forwarding through the gateway is allowed, isolated ports may still reach each other via the gateway, so pair the design with ACLs.

## Hands-on

Open Lab 20: PC1 (separate), PC2 (group 1), PC3 (principal). After configuration PC1 cannot reach PC2, but both reach PC3. Then put PC1/PC2 into the same `port-isolate` group and observe the same result; finally make PC3 separate and see whether it becomes isolated too.
