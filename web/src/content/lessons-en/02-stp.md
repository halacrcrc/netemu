---
title: "STP: Redundancy Without Loops"
order: 2
topic: stp
lab: stp-redundancy
---

For reliability, switches are often cabled together with two links — but Layer 2 has no TTL, so broadcast frames replicate endlessly around the loop and create a **broadcast storm**. STP (Spanning Tree Protocol) logically blocks redundant ports, pruning a physical topology that contains loops into a loop-free tree.

## Three roles

- **Root bridge**: The brain of the network, elected by comparing bridge IDs (priority plus MAC address, lower wins).
- **Root port (RP)**: On every non-root bridge, the port with the lowest cost toward the root bridge.
- **Designated port (DP)**: On each link, the port responsible for forwarding.
- Ports that lose the election become **alternate ports** and are blocked.

## State machine and convergence time

Ports do not forward immediately. Instead they move through:

```
Blocking → Listening → Learning → Forwarding
           └─ 15s ─┘ └─ 15s ─┘
```

A neighbor is only declared dead after 20 seconds of silence (Max Age) — which is why classic STP takes 30 to 50 seconds to converge after a failure. **Changing the root bridge makes every port in the network run through the state machine again**; that is not a bug, it is the real cadence of 802.1D.

## What a broadcast storm looks like

In this simulator, run `stp disable` on both switches and then ping again: broadcast frames replicate exponentially around the loop and the "dropped" counter in the status bar spikes instantly. This is exactly why STP must never be turned off on a looped topology.

## Hands-on

Open Lab 3, "Redundant Links and STP": observe the blocked port, change priority to win the root bridge election, and shut down the primary link to watch automatic convergence; with capture enabled you can see a BPDU every 2 seconds.
