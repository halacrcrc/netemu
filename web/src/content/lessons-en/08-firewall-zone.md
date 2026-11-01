---
title: "Firewalls: Security Zones and Stateful Policy"
order: 8
level: IP
topic: security
lab: firewall-zone
---

A router ACL is stateless: permit the outbound direction and you still need a rule for the reply. The big upgrade in a firewall is **security zones plus stateful sessions** — you describe only who may initiate to whom, and replies are permitted automatically by the session table.

## Zones are the first abstraction

Interfaces are grouped into zones by trust level. Three common ones:

- **trust**: the internal network, highest trust;
- **untrust**: the Internet, lowest trust;
- **dmz**: servers published to the outside, in between.

Traffic inside one zone is permitted by default, while **traffic between zones is denied by default**. Policy is therefore about opening holes between zone pairs, not writing a rule per IP address.

```
Huawei/H3C:
firewall zone trust
 add interface g0/0/0
firewall zone untrust
 add interface g0/0/1
Cisco:
zone security trust
zone security untrust
interface g0/0/0
 zone-member security trust
```

## Security policy and stateful sessions

Policies are matched in order and the first hit wins. After permitting `trust → untrust`, connections initiated from inside create a **session entry**; replies match that session and pass without a reverse policy. Conversely, with no `untrust → trust` policy, outside-initiated connections are denied by default — precisely what separates a firewall from a bidirectional ACL.

```
Huawei/H3C:
security-policy
 rule name allow-out
  source-zone trust
  destination-zone untrust
  action permit
Cisco:
zone-pair security ZP
 source trust
 destination untrust
 action permit
```

Verify with `display security-policy` / `show zone-pair security` for rules and `display firewall zone` / `show zone security` for interface membership.

## Hands-on

Open Lab 10: PC1 is in trust and SRV is in untrust. First have SRV ping PC1 — it should fail; then PC1 pings SRV — it should succeed, with the reply riding the session table. Change permit to deny and try again to see the "default deny" boundary.
