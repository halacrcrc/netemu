---
title: 生成树的三张面孔：PVST+ · MSTP · MSTP
order: 20
level: MAP
topic: stp
lab: stp-redundancy
---

二层环路没有 TTL，广播帧会无限复制。**生成树**通过阻塞冗余端口把有环拓扑剪成无环树。三家厂商都支持标准 STP/RSTP/MSTP，但思科额外有按 VLAN 计算的 PVST+ 家族，这也是跨厂商组网最容易踩坑的地方。

## 三家叫什么

```
思科     PVST+（每 VLAN 一棵树，私有）
         Rapid-PVST+（快速收敛版）
         MSTP（802.1s，标准，可与华为/华三互通）
华为     STP / RSTP / MSTP
华三     STP / RSTP / MSTP
```

- **PVST+**：每个 VLAN 独立选根、独立阻塞，能做按 VLAN 的负载分担，但 VLAN 多时 BPDU 数量大；
- **MSTP**：把多个 VLAN 映射到少量实例（instance），每个实例一棵树，兼顾负载分担与开销；
- **互通**：思科与华为/华三对接时，双方都应使用 MSTP，并保证**域名、修订级别、VLAN-实例映射**一致，否则会各自算树。

## 真实命令参考

```
思科：
spanning-tree mode rapid-pvst
spanning-tree vlan 10 priority 4096
spanning-tree vlan 10 root primary

华为：
stp mode mstp
stp instance 1 priority 4096
stp root primary

华三：
stp mode mstp
stp instance 1 priority 4096
stp root primary
```

优先级必须是 4096 的倍数，数值越小越优先；`root primary` 是宏命令，会把优先级设到一个足以抢根的值。

## 在本模拟器里

模拟器实现 STP/RSTP/MSTP（MSTP 按实例独立计算，但不做域名/修订级别协商）。因此本课的命令对照表聚焦三家都支持的基础操作：优先级、端口成本、关闭/查看。

```
stp priority 4096
stp cost 100
display stp brief
```

## 动手

打开实验三：两台交换机双链路成环，先观察阻塞端口；把 SW1 优先级调到 4096 抢根，再看端口角色变化；shutdown 主链路，等收敛后 PC1 仍能 ping 通 PC2。若要做按 VLAN 负载分担，就在 MSTP 中把不同 VLAN 映射到不同实例。
