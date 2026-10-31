---
title: VLAN 与 Trunk：广播域的隔离
order: 1
topic: vlan
lab: office-l2
---

交换机的每个端口默认都在同一个广播域里——一台主机发 ARP 广播，全网都收到。**VLAN 把一台物理交换机在逻辑上切成多台**，每个 VLAN 是一个独立的广播域。

## 两个核心概念

- **Access 口**：连接主机，只属于一个 VLAN，收发不带标签的帧。
- **Trunk 口**：连接交换机，同时承载多个 VLAN，帧带 802.1Q 标签（原生 VLAN 除外）。

## 为什么需要 Trunk

财务部（VLAN10）的主机分散在两台交换机上时，两台交换机之间必须传递 VLAN 10 的流量——但只用一条线。Trunk 在帧上打 4 字节的 802.1Q 标签来区分 VLAN。

关键差异：**思科 Trunk 默认放行全部 VLAN，华为/华三默认只放行 VLAN 1**——必须显式放行：

```
华为/华三：
 port link-type trunk
 port trunk allow-pass vlan 10
思科：
 switchport mode trunk
 switchport trunk allowed vlan 10
```

## VLAN 间怎么通信

不同 VLAN 即使网段相同也无法互通（二层隔离）。要通信需要三层设备：

- 三层交换机上给每个 VLAN 配一个 **SVI**（华为称 Vlanif）作为网关；
- 或者路由器用**单臂路由**（子接口 + 802.1Q）。

本模拟器的实验二演示 SVI 方案：跨 VLAN 通信后回包 TTL 会减 1——这正是"经过了一次三层转发"的证据。

## 动手

打开实验一「二层办公网（VLAN 隔离）」：划分 VLAN、配置 Trunk，用 ping 验证同 VLAN 互通、跨 VLAN 隔离；选中链路开启抓包，观察 Trunk 上的 802.1Q 标签。
