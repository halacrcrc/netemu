---
title: 无线园区：CAPWAP、SSID 与漫游
order: 11
level: IP
topic: wlan
lab: wlan-advanced
---

企业 Wi-Fi 不是"每台 AP 单独配一遍"，而是**AC 集中管理 + AP 只做射频**。AP 上电后通过 **CAPWAP** 找到 AC、下载配置、建立隧道；SSID、安全策略、业务 VLAN 全部在 AC 上统一下发。

## CAPWAP 的两条隧道

- **控制隧道**：AP 与 AC 之间传配置、心跳、AP 状态；
- **数据隧道**：无线客户端的数据帧封装后送到 AC（集中转发），或由 AP 本地桥接（本地转发）。

本模拟器采用本地桥接模型：AP 把无线终端桥接到有线业务 VLAN，因此 AP 的上行口必须是 **Trunk**，并放行对应 VLAN。

## SSID 与安全

一个 SSID 就是一张"无线名片"，背后绑定三件事：**认证方式、口令、业务 VLAN**。WPA2-PSK 用预共享密钥，适合企业内网；WPA3-SAE 用对等实体认证，能抵御离线字典攻击，适合访客或高安全场景。

```
AC1：
wlan ssid CORP
wlan security wpa2 psk Admin@123
wlan vlan 10
wlan ssid GUEST
wlan security wpa3 sae Guest@123
wlan vlan 20
AP1：
capwap ac 10.0.0.2
```

多 SSID 的关键不是"能连上"，而是**隔离**：CORP 落在 VLAN10、GUEST 落在 VLAN20，即使同一台 AP，两个 SSID 的终端也不能二层互通。

## 漫游：换 AP 不换 IP

终端移动时，AC 根据 AP 优先级与信号质量把它从 AP1 切到 AP2。因为业务 VLAN 与网关不变，终端 IP 不变，已有连接可以继续——这就是**二层漫游**。`wifi roam` 后 `display wlan` / `show wlan` 应看到终端落在新 AP 上。

## 动手

打开实验十四：AC1 下发 CORP（WPA2/VLAN10）与 GUEST（WPA3/VLAN20），AP1/AP2 都注册到 AC。PC1 连 CORP、PC2 连 GUEST，验证各自服务器可达、跨 SSID 不通；再让 PC1 执行 `wifi roam`，确认漫游后业务仍通。
