---
title: "Network Automation and SDN: From CLI to Intent"
order: 33
level: IE
topic: automation
lab: campus-capstone
---

When a network grows from dozens of devices to thousands, logging into each one and typing commands is slow and error-prone. **Network automation** manages devices with programs, templates, and APIs; **SDN** pulls the control plane out of devices and centralizes it in a controller.

## The three planes

- **Data plane**: forwards packets (ASICs/chips);
- **Control plane**: computes routes and runs protocols (OSPF/BGP/STP);
- **Management plane**: configuration, monitoring, alarms (SSH/SNMP/NETCONF).

Traditional devices combine all three; SDN moves the control plane into a controller and turns devices into programmable forwarders that receive instructions over southbound interfaces.

## The automation toolbox

```
Data formats   JSON, XML, YAML
Southbound     NETCONF, RESTCONF, gNMI, OpenFlow
Model language YANG (describes configuration and state)
Collection     SNMP polling, gNMI Telemetry streaming
Orchestration  Ansible (agentless), Python, Terraform
Controllers    iMaster NCE, Cisco DNA Center, OpenDaylight
```

- **NETCONF**: over SSH, XML configuration, candidate/commit with rollback;
- **RESTCONF**: maps YANG models to RESTful APIs, friendlier with JSON;
- **Telemetry**: devices push data continuously, replacing minute-level SNMP polling with second-level streams;
- **Ansible**: YAML playbooks describe the desired state, execute idempotently, and need no agent on the device.

## From configuration to intent

Automation maturity has four levels:

1. **Scripting**: commands in scripts, executed device by device;
2. **Templating**: Jinja2/variables generate configuration, reducing typos;
3. **Orchestration**: a controller pushes configuration with validation and rollback;
4. **Intent**: describe *what* you want (for example, "guest network isolated and rate-limited") and the system translates it into device configuration and continuously verifies it.

A **digital twin** validates changes in simulation before rollout, and Telemetry closes the loop — key to reducing change risk in large networks.

## In this simulator

The simulator focuses on per-packet forwarding and CLI behavior and does not implement NETCONF/YANG controllers. You can still use a Python script against the page's `window.__netemu.exec` to batch commands and experience the first step of programmatic configuration.

## Hands-on

Open Lab 9 and write an "intent checklist": what is the desired state for VLANs, gateways, OSPF, NAT, and ACLs? Then consider: with a controller, which steps can run in parallel, which must be serial, and how would a failure roll back? That checklist is the input to orchestration.
