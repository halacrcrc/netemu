---
title: "Network Operations and Automation Basics"
order: 36
level: IA
topic: ops
lab: campus-capstone
---

HCIA-Datacom V2.0 makes **network operations** and **automation** separate modules. Configuring a device is only the start: proving the network is healthy, locating faults quickly, and handing repetitive work to programs is the daily job.

## Three operational basics

1. **Observe state**: interface up/down, IP, routes, neighbors, CPU/memory;
2. **Verify**: ping for reachability, traceroute for path, captures for packets;
3. **Keep evidence**: configuration backups, logs, and change records so you can compare before and after a change.

```
ping 10.0.0.2
show ip interface brief
show ip route
show running-config
```

## A practical troubleshooting order

- **Physical**: cable, optics, interface up/down;
- **Layer 2**: VLAN, trunk permissions, MAC table, STP state;
- **Layer 3**: IP/mask, gateway, routing table, ARP;
- **Policy**: ACL, NAT, firewall policy, QoS;
- **Application**: DNS, ports, service process.

Work bottom-up; it beats changing configuration on a hunch. Change one variable at a time, verify immediately, and record what you did.

## Where automation starts

- **Data formats**: JSON/XML/YAML;
- **Interfaces**: batch SSH, NETCONF/RESTCONF, SNMP/Telemetry;
- **Tools**: Python, Ansible, controllers (iMaster NCE / DNA Center);
- **Goal**: turn "template + variables" into repeatable, rollback-capable workflows.

This simulator exposes `window.__netemu.exec`, so you can batch CLI commands from a script and experience the first step of programmatic configuration; full controllers and NETCONF are covered in the IE automation course.

## Hands-on

Open Lab 9 and ping hop by hop in the order physical → Layer 2 → Layer 3 → policy → application, recording each result. Then write the configuration steps as a reusable checklist (VLAN/gateway/OSPF/NAT) and decide which can be templated and which need verification first.
