---
title: "Intelligent Network Operations and Structured Troubleshooting"
order: 42
level: IE
topic: ops
lab: campus-capstone
---

HCIE-Datacom V2.0 makes **intelligent network operations** a separate module (14% of the written exam), and the lab exam added a troubleshooting section. Knowing protocols is not enough; you must prove the network is healthy and locate problems quickly before and after changes.

## From polling to streaming

- **SNMP**: minute-level polling, good for trends and alarms;
- **Telemetry (gNMI)**: devices push metrics and streams continuously at second-level granularity;
- **Syslog/NetFlow/sFlow**: logs and flow sampling to find "who is consuming bandwidth" and which session is abnormal;
- **AIOps**: baseline learning, anomaly detection, and root-cause correlation, collapsing alarm storms into a few actionable events.

## Digital twins and change

Simulate in a **digital twin** before a change: which adjacencies will drop, which routes will change; during the change, execute step by step with live validation; after the change, compare before/after metrics. Without simulation, at least prepare **rollback configuration** and a maintenance window.

## Structured troubleshooting

1. **Scope it**: one user, one subnet, or the whole network? When did it start? Was it working before a change?
2. **Verify by layer**: physical → Layer 2 → Layer 3 → policy → application;
3. **Compare evidence**: routing table, ARP/MAC, session table, captures, log timeline;
4. **Hypothesis and test**: change one variable at a time and verify immediately;
5. **Restore and review**: restore service first, find root cause second, add monitoring/automation checks last.

Common "fake faults": DNS failure, MTU mismatch, asymmetric routing, NAT session aging, firewall state timeout.

## Tools and commands

```
display interface brief     show ip interface brief
display ip routing-table    show ip route
display ospf peer           show ip ospf neighbor
display mac-address         show mac address-table
```

In a capture, watch both the **control plane** (BPDU/OSPF/BGP) and the **data plane** (ICMP/TCP) to distinguish "protocol did not converge" from "forwarding was blocked by policy".

## Hands-on

Open Lab 9 and create three faults: a trunk missing one VLAN, a wrong OSPF wildcard, and NAT configured inside-only. Troubleshoot each with the five steps and record the evidence and conclusion. Finally, turn the three checklists into one troubleshooting sheet.
