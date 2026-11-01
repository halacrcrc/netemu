---
title: "F5 LTM: Sharing Traffic Across a Server Farm"
order: 14
level: IE
topic: ltm
lab: f5-ltm
---

When one web server cannot keep up, the obvious fix is more servers — but users remember only one address. That is where a **load balancer** comes in: it exposes a virtual IP (VIP) to the outside and distributes connections across a pool of servers.

## The VIP / Pool / Member model

- **Virtual Server (VS)**: the public VIP plus port, for example 10.0.0.100:80;
- **Pool**: a group of back-end servers, for example 10.0.1.10:80 and 10.0.2.11:80;
- **Member**: one server in the pool, which can be marked healthy or taken out individually;
- **Algorithm**: round robin is the simplest; least connections or hashing are alternatives.

```
LB1:
ltm pool WEB member 10.0.1.10 80
ltm pool WEB member 10.0.2.11 80
ltm virtual VS vip 10.0.0.100 80 pool WEB
```

## How the addresses are rewritten

Clients know only the VIP. When a connection arrives, the LTM does two things:

- **DNAT**: rewrite the destination from the VIP to the selected member's IP;
- **SNAT / return path**: record the session so the member's replies return to the right client and leave from the VIP.

As a result, servers may see a source address other than the client's real one, while the client always believes it is talking to the VIP. The session table in `display ltm` / `show ltm` shows the "client → VIP → member" mapping directly.

## Hands-on

Open Lab 19: LB1 has three interfaces for the client and two servers. After configuring the pool and VS, run `tcp 10.0.0.100 80` twice from PC1 — both should connect. The Sessions list in `display ltm` should contain two entries pointing at different members, proving round robin. Shut down one server and connect again to see whether traffic lands only on the surviving member.
