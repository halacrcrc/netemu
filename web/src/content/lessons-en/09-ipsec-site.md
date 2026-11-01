---
title: "IPsec: Moving Branch Private Networks Across the Internet"
order: 9
level: IP
topic: ipsec
lab: ipsec-site
---

Two branches each have a private network but can only reach each other over the public Internet. A **site-to-site IPsec VPN** builds an encrypted tunnel between their public egresses, carrying all traffic between 192.168.1.0/24 and 192.168.2.0/24 inside it, so the Internet only sees the two egress IPs talking.

## Three key elements

- **Peer addresses**: the public IP at each end of the tunnel; they must be reachable from each other;
- **Protected subnets**: which source/destination networks use the tunnel (the "interesting traffic"); everything else still uses the Internet;
- **Security parameters**: encryption, authentication, and keys — real devices negotiate these with IKE, while this simulator simplifies to a static tunnel that validates endpoints and subnets.

Real IPsec runs in two steps: IKE first negotiates a secure channel over the untrusted network (phase 1), then uses it to negotiate ESP parameters (phase 2), after which data is carried in ESP. In tunnel mode the entire original IP packet is encrypted and wrapped in a new public IP header.

```
Huawei/H3C:
ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2
 local-subnet 192.168.1.0 24
 remote-subnet 192.168.2.0 24
Cisco:
ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2
 local-subnet 192.168.1.0 24
 remote-subnet 192.168.2.0 24
```

The two ends must **mirror each other**: R1's local is R2's remote, and R1's local-subnet is R2's remote-subnet. Configure only one side and the tunnel never comes up.

## Hands-on

Open Lab 11: configure T1 on R1 and R2 with mirrored subnets. `display ipsec tunnel` / `show ipsec tunnel` should list the protected subnets, and PC1 should ping 192.168.2.10. Capture the R1–R2 link: the outer packet is IPsec over 10.0.0.x, and the inner packet is the original ICMP from 192.168.1.x.
