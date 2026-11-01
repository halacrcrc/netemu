---
title: "Enterprise Wi-Fi: CAPWAP, SSIDs, and Roaming"
order: 11
level: IP
topic: wlan
lab: wlan-advanced
---

Enterprise Wi-Fi is not "configure every AP separately". It uses a **controller (AC) plus thin APs**: when an AP powers on it finds the AC over **CAPWAP**, downloads its configuration, and establishes a tunnel; SSIDs, security policy, and service VLANs are all provisioned from the AC.

## The two CAPWAP tunnels

- **Control tunnel**: carries configuration, heartbeats, and AP state between AP and AC;
- **Data tunnel**: carries client frames to the AC (centralized forwarding) or bridges them locally at the AP (local forwarding).

This simulator uses local bridging: the AP bridges wireless clients into the wired service VLAN, so its uplink must be a **trunk** that permits that VLAN.

## SSIDs and security

An SSID is the wireless "name card", and it binds three things: **authentication, passphrase, and service VLAN**. WPA2-PSK uses a pre-shared key and suits the corporate network; WPA3-SAE uses simultaneous authentication of equals, resisting offline dictionary attacks — a good fit for guest or high-security use.

```
AC1:
wlan ssid CORP
wlan security wpa2 psk Admin@123
wlan vlan 10
wlan ssid GUEST
wlan security wpa3 sae Guest@123
wlan vlan 20
AP1:
capwap ac 10.0.0.2
```

The point of multiple SSIDs is not just connectivity but **isolation**: CORP lands in VLAN 10 and GUEST in VLAN 20, so even on one AP the two SSIDs cannot reach each other at layer 2.

## Roaming: change AP, keep the IP

As a client moves, the AC switches it from AP1 to AP2 based on AP priority and signal quality. Because the service VLAN and gateway stay the same, the client keeps its IP and existing sessions survive — this is **layer-2 roaming**. After `wifi roam`, `display wlan` / `show wlan` should show the client on the new AP.

## Hands-on

Open Lab 14: AC1 provisions CORP (WPA2/VLAN 10) and GUEST (WPA3/VLAN 20), and both APs join the AC. Connect PC1 to CORP and PC2 to GUEST, verify each server is reachable and that cross-SSID traffic fails, then run `wifi roam` on PC1 and confirm service survives the move.
