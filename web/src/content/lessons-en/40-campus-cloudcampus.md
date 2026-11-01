---
title: "Campus Network Planning and CloudCampus"
order: 40
level: IE
topic: campus
lab: campus-capstone
---

HCIE-Datacom V2.0 gives **campus planning and deployment** the highest weight (43% of the written exam). It tests whole-network design — requirements, capacity, resilience, admission, and operations — not a single command.

## Four design steps

1. **Requirements and capacity**: endpoint count, concurrency, AP density, uplink bandwidth, three-year growth;
2. **Hierarchy and resilience**: access-aggregation-core, dual uplinks with LACP/stacking/M-LAG, VRRP gateways;
3. **Addressing and routing**: VLAN/subnet plan, DHCP/DHCPv6, OSPF or static routes, egress NAT;
4. **Security and admission**: ACLs, firewall zones, 802.1X/NAC, guest isolation, policy following the user.

## Wireless and cloud management

- **CloudCampus**: iMaster NCE manages APs/ACs/switches uniformly and pushes templated configuration;
- **AP placement**: plan for both coverage and capacity, split 2.4/5 GHz, avoid co-channel interference;
- **Roaming**: layer-2 roaming within the same service VLAN and gateway; tunnel or layer-3 roaming across subnets;
- **Policy following the user**: bind policy to identity rather than IP/port so it follows the user.

## Admission and boundaries

- **802.1X**: port authentication; unauthenticated endpoints only get EAP/DHCP/DNS;
- **MAC authentication**: suits printers/IoT; weaker than 802.1X;
- **Portal**: guest access with ACL isolation;
- **GRE/IPsec**: used for branch-to-HQ and Internet interconnect.

## Design checklist

- Does every link have redundancy, and is failover time acceptable?
- Does every VLAN have a gateway, DHCP, ACL, and broadcast suppression?
- Are wireless coverage and capacity sufficient, and roaming seamless?
- Are management and business separated, with logs/Telemetry exported?
- Are changes rollback-capable and configurations backed up?

## Hands-on

Open Lab 9 and treat it as a campus delivery: draw the three-tier topology and address plan, then check link redundancy and each VLAN's gateway and egress. Answer: if an access switch has a single uplink, which services are affected? If all APs hang off one access switch, what is the risk?
