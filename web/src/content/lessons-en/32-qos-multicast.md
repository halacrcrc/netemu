---
title: "QoS and Multicast: Prioritizing Critical Traffic"
order: 32
level: IE
topic: qos
lab: campus-capstone
---

Under congestion, treating every flow "fairly" makes voice stutter and video pixelate. **QoS** gives different services different treatment; **multicast** delivers one stream to many receivers without duplicating it at the source.

## The four QoS actions

```
Classification   Identify traffic by ACL, port, or DSCP
Marking          Set DSCP/CoS so downstream devices can trust it
Policing         Drop or downgrade excess immediately, enforcing a rate
Shaping          Buffer excess and smooth it out, adding delay
Queuing          PQ priority, WFQ fairness, LLQ low-latency queue
```

DSCP occupies 6 bits in the IP header (0–63): **EF=46** for voice, **AF41=34** for video, **CS6/CS7** for protocol control. End-to-end QoS requires every hop to trust and enforce the same markings, or they will be stripped at a boundary.

Congestion avoidance often uses **WRED**: as a queue fills, drop low-priority packets probabilistically to avoid global TCP synchronization.

## Multicast: one stream, many receivers

- **IGMP**: hosts tell the router which group they want (IGMPv2/v3);
- **PIM-SM**: sparse mode builds a shared tree rooted at the RP (`(*,G)`) and switches to the source tree (`(S,G)`) when traffic grows;
- **PIM-SSM**: specifies source and group directly, ideal for video and live streams;
- **RPF check**: multicast packets must arrive on the interface on the shortest path back to the source, otherwise they are dropped — the core loop-prevention rule.

Multicast addresses are `224.0.0.0/4`; `224.0.0.0/24` is link-local protocol space and `239.0.0.0/8` is private multicast.

## In this simulator

The simulator implements **broadcast storm suppression** and **ingress rate limiting**, with drop/suppression counters:

```
Cisco:
storm-control broadcast level 100
rate-limit input 1000
Huawei/H3C:
broadcast-suppression 100
qos car inbound cir 1000
```

Full DSCP queues, WRED, and PIM are not modeled, but the framework — classify, mark, then queue or drop — explains real device behavior.

## Hands-on

Open Lab 9: configure broadcast suppression on an access port, generate broadcast traffic, and watch the drop counter; then rate-limit the uplink and observe excess traffic being dropped. Consider why voice fits LLQ while large downloads fit WFQ.
