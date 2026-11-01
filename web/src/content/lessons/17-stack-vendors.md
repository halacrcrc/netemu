---
title: 堆叠的三张面孔：StackWise/VSS · iStack/CSS · IRF
order: 17
level: MAP
topic: stack
lab: stack-irf
---

堆叠的目标只有一个：**把多台物理交换机变成一台逻辑设备**。对外一个管理 IP、一份配置、一个 STP 根桥；成员之间用专用链路互联，不再跑 STP，也不会互相阻塞。

## 三家叫什么

```
思科        StackWise（Catalyst 接入堆叠）
            VSS（6500/4500 虚拟交换系统）
            StackWise Virtual（Catalyst 9000）
华为        iStack（S 系列接入堆叠）
            CSS（CloudEngine 集群交换系统）
华三        IRF（Intelligent Resilient Framework）
```

虽然名字不同，核心机制一致：**主设备选举 + 配置同步 + 成员间转发**。

- **主设备选举**：比较优先级与成员编号，优先级高者当 Master；
- **成员编号**：决定端口命名（如 G1/0/1 与 G2/0/1），冲突会导致加入失败；
- **堆叠链路**：承载成员间控制与数据流量，带宽要足够，通常做跨板卡/跨设备冗余；
- **分裂（split-brain）**：堆叠链路全断时，两台成员都以为自己是主，真实设备用 MAD（多主动检测）机制抑制。

## 思科 VSS 与华为 CSS 的差异

VSS/CSS 面向框式设备：用万兆链路做 **VSL / 集群链路**，把两台机框虚拟成一台；StackWise/iStack 面向盒式设备，用专用堆叠口或业务口堆叠。配置面都是"一台设备"，但底层链路与故障域不同。

## 在本模拟器里

模拟器用统一的 `stack domain / member / priority` 建模，三家命令相同；同一 domain 的成员间链路自动视为 fabric，STP 旁路，BPDU 直接丢弃。

```
stack domain 10
stack member 1
stack priority 200
```

验证命令：`display stack` / `show stack` 应看到 domain、成员列表与 Master。

## 动手

打开实验十六：SW1 优先级 200 当 Master、SW2 为成员 2，两台都加入 domain 10。PC1 与 PC2 分接不同成员，仍应在同一 VLAN 内互通；`display stack` 确认角色。再思考：如果堆叠链路断开，为什么真实网络需要 MAD，而本模拟器没有建模？
