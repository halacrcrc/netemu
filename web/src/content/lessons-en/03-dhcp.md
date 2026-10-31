---
title: "DHCP: Automatic Address Distribution"
order: 3
topic: dhcp
lab: enterprise
---

Configure IP address, mask, gateway and DNS by hand for every PC, and 50 machines means 200 entries. DHCP lets a host **receive every parameter automatically** the moment it joins the network.

## The four-step DORA dance

```
Client                          Server
  │ ── Discover (broadcast) ──→ │
  │ ←── Offer ─────────────────  │
  │ ── Request (broadcast) ───→ │
  │ ←── Ack ───────────────────  │
```

- During Discover the client **has no address yet**: the source IP is 0.0.0.0 and the destination is 255.255.255.255 — which is also why DHCP across subnets needs a relay.
- The server picks a pool by this rule: **use the pool whose subnet matches the interface that received the request**.

## What the server hands out

Not just an IP and mask: the **default gateway** (option 3) and **DNS** (option 6) matter just as much — without a gateway, the client cannot leave its subnet.

## Leases and conflicts

The server keeps a lease table: an address already handed out is never issued twice (a second host automatically gets the next free one). Real devices also track lease lifetime and renewal (a unicast Request at the 50% mark), while this simulator simplifies leases to never expire.

## Hands-on

Open Lab 4, "Enterprise Internet Access": configure an address pool, enable "Automatic DHCP" in the PC's property panel, and watch it receive 192.168.1.2 and the gateway on its own.
