---
title: WLAN 基础：802.11、AP/AC 与安全
order: 27
level: IA
topic: wlan
lab: wlan-capwap
---

无线不是"把网线换成空气"：共享介质、半双工、信号衰减，决定了 802.11 需要一套自己的接入与安全机制。

## 802.11 家族与频段

- **2.4 GHz**：覆盖好、干扰多，只有 3 个互不重叠信道（1/6/11）；
- **5 GHz**：信道多、干扰少、速率高，覆盖略短；
- **6 GHz（Wi-Fi 6E）**：新增频段，进一步减少拥塞；
- **802.11n/ac/ax**：分别对应 Wi-Fi 4/5/6，ax 引入 OFDMA 与更高阶调制。

无线是**半双工共享介质**，用 CSMA/CA（先听后发、冲突避免）竞争信道，所以"信号满格但很慢"往往是同频干扰或空口拥塞，而不是带宽不够。

## 基本术语

- **SSID**：无线网络名；**BSSID**：AP 的射频 MAC；
- **AP**：无线接入点；**AC**：集中管理 AP 的控制器；
- **CAPWAP**：AP 与 AC 之间的控制/数据隧道；
- **漫游**：终端在 AP 间切换而保持 IP 与业务。

## 安全：从 Open 到 WPA3

```
Open        无认证，任何人可连（仅实验/访客隔离网络）
WEP         已被破解，禁用
WPA/WPA2-PSK 预共享密钥，企业常用 WPA2-Enterprise（802.1X）
WPA3-SAE    对等实体认证，抗离线字典攻击
```

企业里通常把员工 SSID 映射到内部 VLAN、访客 SSID 映射到隔离 VLAN，并在 AC 上统一下发策略。

## 在本模拟器里

AC 下发 SSID、安全与业务 VLAN；AP 通过 `capwap ac` 注册；PC 终端用 `wifi connect` 关联，`wifi status` 查看状态。AP 上行口要配 Trunk 放行业务 VLAN。

```
wlan ssid CORP
wlan security wpa2 psk Admin@123
wlan vlan 10
capwap ac 10.0.0.2
```

## 动手

打开实验十三：AC1 配 CORP（WPA2/VLAN10），AP1 注册后 PC1 用正确口令关联并访问有线服务器；PC2 用错误口令应失败。再打开实验十四，体验多 SSID 隔离与漫游。
