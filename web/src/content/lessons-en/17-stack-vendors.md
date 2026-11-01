---
title: "Stacking, Three Names: StackWise/VSS · iStack/CSS · IRF"
order: 17
level: MAP
topic: stack
lab: stack-irf
---

Stacking has one goal: **turn several physical switches into one logical device** — one management IP, one configuration, one STP root. Members interconnect over dedicated links that do not run STP and never block each other.

## What each vendor calls it

```
Cisco       StackWise (Catalyst access stacking)
            VSS (Virtual Switching System, 6500/4500)
            StackWise Virtual (Catalyst 9000)
Huawei      iStack (S series access stacking)
            CSS (Cluster Switch System, CloudEngine)
H3C         IRF (Intelligent Resilient Framework)
```

Different names, same core mechanics: **master election + configuration sync + inter-member forwarding**.

- **Master election**: priority and member ID decide; the higher priority becomes Master;
- **Member ID**: determines port naming (G1/0/1 vs G2/0/1); a duplicate ID prevents a switch from joining;
- **Stack links**: carry inter-member control and data traffic, so they need enough bandwidth and redundancy;
- **Split-brain**: if all stack links fail, both members may believe they are master; real devices use MAD (multi-active detection) to suppress the duplicate.

## VSS/CSS versus StackWise/iStack

VSS and CSS target chassis devices: high-speed links form the **VSL / cluster link** and virtualize two chassis into one. StackWise and iStack target fixed switches and use dedicated stacking ports or business ports. Both present one configuration plane, but their link types and failure domains differ.

## In this simulator

The simulator models everything with `stack domain / member / priority`, identical across dialects. Links between members of the same domain are treated as fabric, STP is bypassed, and BPDUs are dropped.

```
stack domain 10
stack member 1
stack priority 200
```

Verify with `display stack` / `show stack` to see the domain, members, and Master.

## Hands-on

Open Lab 16: make SW1 the Master with priority 200 and SW2 member 2 in domain 10. PC1 and PC2 attach to different members but still share a VLAN and can ping each other. Then consider: if the stack link breaks, why does a real network need MAD while this simulator does not model it?
