---
title: "QoS: Classification, Marking, Queuing, and Congestion Avoidance"
order: 46
level: IP
topic: qos
lab: campus-capstone
---

Under congestion, treating every flow "fairly" makes voice stutter, video pixelate, and business-critical traffic fail first. **QoS** does not add bandwidth; it gives different services different treatment within the bandwidth you have.

## Five actions

```
Classification   Identify traffic by ACL, port, DSCP, or application
Marking          Set DSCP/CoS so downstream devices can trust it
Policing         Drop or downgrade excess immediately, enforcing a rate
Shaping          Buffer excess and smooth it out, adding delay
Queuing          PQ priority, WFQ fairness, LLQ low-latency queue
```

Order matters: **classify, mark, police/shape at boundaries, queue at congestion points**. If every hop does not trust the same markings, QoS fails at the boundary.

## DSCP and the trust boundary

DSCP occupies 6 bits in the IP header (0–63). Common values:

```
EF = 46      Voice, low latency and jitter
AF41 = 34    Video
AF11 = 10    Bulk data
CS6 / CS7    Protocol control
BE = 0       Best effort
```

Endpoints may mark packets arbitrarily, so access switches usually re-mark or trust only specific ports; beyond the trust boundary, the core simply queues by DSCP.

## Policing, shaping, and queues

- **Policing**: drop or downgrade excess immediately; suits ingress rate enforcement;
- **Shaping**: buffer excess and send smoothly; suits egress toward a carrier committed rate;
- **PQ/LLQ**: voice uses a priority queue scheduled first under congestion;
- **WFQ**: other traffic shares bandwidth by weight;
- **WRED**: probabilistically drops low-priority packets as a queue fills, preventing all TCP flows from backing off together.

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

Full DSCP queues, WRED, and MQC are not modeled, but the framework — classify, mark, then queue or drop — matches real devices.

## Design checklist

1. Define service classes (voice/video/office/bulk/guest);
2. Classify and mark at access; trust DSCP in the core;
3. Configure LLQ + WFQ + WRED at congestion points;
4. Shape egress to the carrier's committed rate;
5. Verify policy hits with `show qos` / `display qos` counters.

## Hands-on

Open Lab 9: configure broadcast suppression on an access port, generate broadcast traffic, and watch the drop counter; then rate-limit the uplink and observe excess traffic being dropped. Then decide which queues voice, video, and bulk downloads should use.
