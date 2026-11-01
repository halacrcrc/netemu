---
title: "Networking Fundamentals: OSI, TCP/IP, and Encapsulation"
order: 22
level: IA
topic: basics
lab: office-l2
---

Every networking technology rests on the same layered model. Once you understand which layers data crosses and what header each layer adds, VLANs, routing, and NAT stop being commands to memorize.

## Two models, one map

OSI has seven layers for teaching; TCP/IP has four or five for engineering:

```
OSI                        TCP/IP            Typical protocols / devices
Application/Presentation/Session  Application   HTTP, DNS, SSH
Transport                  Transport         TCP, UDP
Network                    Network           IP, ICMP, ARP (often treated as L2)
Data link                  Network access    Ethernet, PPP, VLAN
Physical                                     Twisted pair, fiber, wireless
```

- **Switches** read the Layer 2 MAC header; **routers** read the Layer 3 IP header;
- Each layer talks to its peer and treats lower layers as a service;
- Encapsulation adds headers top-down; decapsulation strips them bottom-up.

## What actually happens during a ping

1. The application generates an ICMP Echo Request;
2. The network layer adds an IP header and looks up the route for the egress interface and next hop;
3. If the next-hop MAC is unknown, **ARP** broadcasts "who owns this IP?";
4. The data link layer adds an Ethernet header and transmits;
5. Each router decrements **TTL**; at zero the packet is dropped and an ICMP Time Exceeded is returned;
6. The destination replies with Echo Reply, following the return route.

ARP works only inside a **broadcast domain** — which is why "cross-subnet ping failures" start with checking the gateway and ARP. TCP is connection-oriented and reliable (three-way handshake, sequence numbers, retransmission); UDP is connectionless and cheap, a good fit for DNS, voice, and video.

## In this simulator

In any device terminal:

```
ping 192.168.1.1
show ip interface brief
show ip route
```

A capture shows each layer's header: Ethernet MACs, IP addresses and TTL, and ICMP type. Matching TTL changes to hop count is the fastest way to internalize per-packet forwarding.

## Hands-on

Open Lab 1: two PCs in the same VLAN can communicate because ARP broadcasts are flooded within one broadcast domain; cross-VLAN traffic fails because of Layer 2 isolation. Then open Lab 2's SVI and watch TTL decrease by one across VLANs — proof that the packet really went through a Layer 3 hop.
