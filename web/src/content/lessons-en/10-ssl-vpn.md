---
title: "SSL VPN: Remote Access Without a Site Tunnel"
order: 10
level: IP
topic: sslvpn
lab: sslvpn-remote
---

IPsec fits fixed site-to-site links; an employee at home or in a hotel is better served by **SSL VPN**. Built on the same TLS used by browsers, it authenticates the user, assigns a **virtual IP**, and then makes the internal network reachable as if the user were in the office.

## How it differs from IPsec

- **IPsec**: site to site, two gateways trust each other long-term, protecting traffic between subnets;
- **SSL VPN**: user to gateway, authenticating per person and assigning addresses per session — a natural fit for mobile work;
- **Default deny still applies**: the outside interface is in untrust and the inside in trust, so SSL VPN is the one permitted entrance.

## Three configuration threads

1. **Virtual IP pool**: the internal range assigned to remote clients; it must not overlap existing subnets;
2. **AAA authentication**: local users or an external server decides who may connect;
3. **Gateway policy**: permit the SSL VPN service and let virtual IPs reach internal resources.

```
Huawei/H3C:
aaa
 local-user admin password cipher Admin@123
sslvpn virtual-ip 10.10.0.0 24
Cisco:
username admin privilege 15 secret Admin@123
sslvpn virtual-ip 10.10.0.0 24
```

From the PC shell, run `sslvpn connect 203.0.113.1 admin Admin@123`; the client receives 10.10.0.2 on success. `display sslvpn session` / `show sslvpn session` lists the virtual IP, the client's real IP, and the session state.

## Hands-on

Open Lab 12: FW1 has 203.0.113.1 outside and 192.168.1.1 inside, with pool 10.10.0.0/24. **Before connecting**, PC1 should fail to ping 192.168.1.10; after `sslvpn connect` it should succeed. Disconnect and ping again — the session is gone and access is denied once more, proving the tunnel is the only path.
