---
title: "WLAN Basics: 802.11, AP/AC, and Security"
order: 27
level: IA
topic: wlan
lab: wlan-capwap
---

Wireless is not "Ethernet over air". A shared medium, half-duplex operation, and signal attenuation force 802.11 to define its own access and security mechanisms.

## The 802.11 family and bands

- **2.4 GHz**: good coverage, heavy interference, only three non-overlapping channels (1/6/11);
- **5 GHz**: more channels, less interference, higher rates, slightly shorter range;
- **6 GHz (Wi-Fi 6E)**: a new band that further reduces congestion;
- **802.11n/ac/ax**: Wi-Fi 4/5/6; ax adds OFDMA and higher-order modulation.

Wireless is a **half-duplex shared medium** using CSMA/CA (listen before transmit, avoid collisions). "Full bars but slow" usually means co-channel interference or airtime congestion, not insufficient bandwidth.

## Basic terms

- **SSID**: the wireless network name; **BSSID**: the AP radio's MAC;
- **AP**: access point; **AC**: controller managing APs centrally;
- **CAPWAP**: the control/data tunnel between AP and AC;
- **Roaming**: a client moves between APs while keeping its IP and sessions.

## Security: from Open to WPA3

```
Open         No authentication; lab or isolated guest only
WEP          Broken; never use
WPA/WPA2-PSK Pre-shared key; enterprises also use WPA2-Enterprise (802.1X)
WPA3-SAE     Simultaneous authentication of equals, resists offline dictionary attacks
```

Enterprises typically map the employee SSID to an internal VLAN and the guest SSID to an isolated VLAN, with policy pushed from the AC.

## In this simulator

The AC provisions SSID, security, and service VLAN; the AP registers with `capwap ac`; PCs associate with `wifi connect` and inspect state with `wifi status`. The AP uplink must be a trunk permitting the service VLAN.

```
wlan ssid CORP
wlan security wpa2 psk Admin@123
wlan vlan 10
capwap ac 10.0.0.2
```

## Hands-on

Open Lab 13: configure CORP (WPA2/VLAN 10) on AC1; after AP1 registers, PC1 associates with the correct passphrase and reaches the wired server, while PC2's wrong passphrase fails. Then open Lab 14 to explore multi-SSID isolation and roaming.
