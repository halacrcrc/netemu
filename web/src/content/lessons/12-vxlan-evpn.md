---
title: VXLAN 与 EVPN：跨机房延伸二层
order: 12
level: IE
topic: vxlan
lab: evpn-dc
---

虚拟机迁移要求"IP 不变、网关不变"，也就要求两个机房在**同一个二层域**里。但传统 VLAN 只有 4094 个、且不能跨三层网络延伸。**VXLAN** 的做法是：把原始以太帧整个装进 UDP 报文，在三层 Underlay 上跑，等于给二层网络插上了三层的翅膀。

## VTEP、VNI 与封装

- **VTEP**：VXLAN 隧道端点，通常是数据中心的叶交换机；
- **VNI**：24 位的网络标识，一个 VNI 对应一个二层域（可映射到本地 VLAN）；
- **封装**：外层 IP/UDP（目的端口 4789）→ VXLAN 头（含 VNI）→ 原始以太帧。

同一 VNI 内的广播、未知单播会被复制到所有对端 VTEP；已知单播则直接走对应隧道。因为外层是普通 IP，Underlay 只需要三层可达，不必是大二层。

```
DSW1：
vlan batch 10 100
interface vlanif 100
 ip address 10.0.0.1 24
vxlan local 10.0.0.1
vxlan vni 10000 vlan 10
vxlan peer 10.0.0.2
```

## 从静态 Peer 到 EVPN

手工配 `vxlan peer` 在两张交换机时没问题，规模一大就变成 N² 的配置地狱。**EVPN** 用 BGP 作为控制面，把"我是谁、我有哪些 VNI"做成路由（简化 Type-3）通告出去；对端学到后自动加入 VTEP 列表，数据面随即可以建隧道。

```
DSW1：
evpn enable
evpn peer 10.0.0.2
```

控制面负责"发现"，数据面仍然是 VXLAN 封装——两者分工清晰，这也是 EVPN 成为数据中心主流方案的原因。

## 动手

先打开实验十五：手工指定 peer，H1 与 H2 同网段、跨 DC 直接 ping 通；抓包能看到外层 10.0.0.x、内层原始以太帧。再打开实验十八：去掉静态 peer，改用 `evpn enable` + `evpn peer`，等 2 秒通告后 `display evpn` / `display vxlan` 应自动出现远端 VTEP，H1 仍能 ping 通 H2。
