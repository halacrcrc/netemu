---
title: "ACL and NAT: Gatekeeper and Translator"
order: 4
topic: acl
lab: enterprise
---

## ACL: the gatekeeper on an interface

An ACL is a set of rules matched in order, and **the first match wins**; if nothing matches, an invisible "implicit deny all" applies. The rule of thumb for writing ACLs: "permit what you need first, and everything else is denied by default."

Two easy mistakes:

- **Wildcard masks work the opposite way** from subnet masks: 0 means the bit must match, 1 means ignore it. `0.0.0.255` matches a /24 subnet, while `0` matches a single host exactly.
- Number ranges: on Huawei/H3C, 2000-2999 are basic ACLs (source address only) and 3000-3999 are advanced (source plus destination plus protocol); on Cisco the equivalents are 1-99 and 100-199.

An ACL only takes effect once it is **applied to an interface in the in or out direction**: inbound is evaluated before routing, outbound after routing. The matched counter in `display acl all` shows directly how many packets were blocked.

## NAT: translator between private and public networks

Internal hosts use private addresses (192.168.x.x and similar), and traffic heading to the public internet must be translated into public addresses. Two approaches:

- **Easy-IP (PAT)**: The outbound source address is rewritten to the egress interface's public IP, and ports or ICMP session IDs distinguish the internal hosts — replies are restored from the translation table. Only internal hosts can start sessions; outside traffic cannot get in.
- **Static NAT**: One private address maps permanently to one public address, and outside hosts can initiate connections — use this to publish internal servers.

Translation works in both directions: outbound traffic rewrites the **source**, return traffic rewrites the **destination**. `show ip nat translations` lists every session.

## Hands-on

Open Lab 4, "Enterprise Internet Access": get an address via DHCP, reach the internet through NAT (ping the public server and inspect the translation table), then optionally add an ACL to block traffic and observe the matched counter.
