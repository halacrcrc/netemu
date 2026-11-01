---
title: 数据中心 VXLAN/EVPN 的三家实现
order: 21
level: MAP
topic: vxlan
lab: evpn-dc
---

VXLAN 用"MAC-in-UDP"把二层帧封装到三层网络，EVPN 用 BGP 做控制面自动发现 VTEP。标准是同一套，但思科 Nexus、华为 CloudEngine、华三的数据中心交换机在**配置模型和命令**上差异很大。

## 三家怎么组织配置

```
思科 Nexus：
 feature vn-segment-vlan-based
 vlan 10
  vn-segment 10000
 interface nve1
  source-interface loopback0
  member vni 10000
   ingress-replication protocol bgp
 EVPN 通过 address-family l2vpn evpn 通告

华为 CloudEngine：
 bridge-domain 100
  vxlan vni 10000
 interface Nve1
  source 10.0.0.1
  vni 10000 head-end peer-list 10.0.0.2
 EVPN 通过 BGP EVPN 邻居学习

华三：
 vsi 100
  vxlan 10000
  tunnel 1
 interface Vsi-interface 100
  ...
 EVPN 通过 BGP EVPN 地址族通告
```

可以把它归纳成三层：**业务实例（VLAN/BD/VSI）→ VNI 映射 → 隧道端点（NVE/VTEP）**。思科用 `nve1` 接口，华为用 `Nve1`，华三用 `tunnel` + `vsi`；名字不同，封装都是 VXLAN over UDP 4789。

## 控制面：静态 Peer 与 EVPN

- **静态 Peer**：手工指定每个远端 VTEP，适合两台设备的小规模验证；
- **EVPN**：BGP 通告 VTEP 与 VNI（本模拟器简化为 Type-3），对端自动学习并加入隧道列表；
- **分布式网关**：真实数据中心还会用 EVPN 的 Type-2/Type-5 路由做主机路由与三层网关，本模拟器只覆盖二层延伸。

## 在本模拟器里

模拟器用统一模型：`vxlan local / vni / peer` 配置静态隧道，`evpn enable / evpn peer` 开启控制面。三家命令相同，便于把注意力放在"VNI 映射 + 对端发现"这两个核心概念上。

```
vxlan local 10.0.0.1
vxlan vni 10000 vlan 10
vxlan peer 10.0.0.2
evpn enable
evpn peer 10.0.0.2
```

## 动手

先打开实验十五：静态 peer 让 H1/H2 跨 DC 二层互通；再打开实验十八：改用 EVPN，等 2 秒通告后 `display evpn` / `display vxlan` 自动出现远端 VTEP。抓包对比：外层 10.0.0.x + UDP 4789，内层是原始以太帧。
