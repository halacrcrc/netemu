---
title: 跨设备聚合：vPC · M-LAG · DRNI
order: 18
level: MAP
topic: mlag
lab: mlag-dc
---

接入交换机双上行到两台汇聚，传统做法靠 STP 阻塞一条链路。**跨设备链路聚合**让两台汇聚对外表现得像一台 LACP 邻居：两条上行同时转发，故障时无需等 STP 收敛。思科、华为、华三对它的叫法不同，但解决的问题完全一样。

## 三家叫什么

```
思科     vPC（virtual Port Channel，Nexus 数据中心）
华为     M-LAG（Multi-Chassis Link Aggregation，CloudEngine）
华三     DRNI（Distributed Resilient Network Interconnect）
```

核心机制可以归纳为四点：

- **双控制面**：两台设备各自独立跑协议，不合并成一台（这点和堆叠不同）；
- **共享系统标识**：对端 LACP 看到的是同一个 System MAC / 系统 ID；
- **peer-link**：两台设备之间的互联链路，承载跨设备流量与表项同步；
- **水平分割**：从 peer-link 收到的广播/未知单播，不再从跨设备 LAG 发回去，避免环路。

## 真实设备命令参考

不同版本命令略有差异，下面只保留最能体现模型的骨架（具体以各厂商文档为准）。

```
思科 NX-OS（vPC）：
vpc domain 1
 peer-keepalive destination 10.0.0.2
interface port-channel 10
 vpc peer-link
interface port-channel 20
 vpc 20

华为（M-LAG）：
dfs-group 1
 peer-link 1
interface Eth-Trunk 10
 peer-link 1
interface Eth-Trunk 20
 mlag 10

华三（DRNI）：
drni system-mac 0000-0000-0001
drni system-number 1
drni system-priority 100
interface Bridge-Aggregation 10
 drni ip-portal
interface Bridge-Aggregation 20
 drni dr-group 1
```

术语映射：vPC 的 **peer-keepalive** ≈ M-LAG 的 **DFS 组** ≈ DRNI 的 **IPP/DRCP**；vPC 的 **member vPC** ≈ M-LAG 的 **mlag id** ≈ DRNI 的 **DR 组**。

## 和堆叠的区别

- **控制面**：堆叠合并为一台；跨设备聚合两台独立；
- **配置**：堆叠只有一份；跨设备聚合是两份，必须保持一致；
- **故障域**：堆叠链路断开要靠 MAD 防双主；peer-link 断开同样有双主风险；
- **升级**：堆叠可整体或滚动；跨设备聚合可以逐台升级，业务影响更小。

## 在本模拟器里

模拟器用统一的 `mlag domain` 建模：同 domain 的两台交换机共享系统 MAC `02:00:00:<domain>:01/02`，接口执行 `mlag peer-link` 后成为 peer-link；接入交换机的 LACP 双归会被选为同一聚合组。

```
DSW1/DSW2：
mlag domain 1
interface g0/0/2
 mlag peer-link
```

## 动手

打开实验十七：两台 DSW 配 `mlag domain 1`，SW1 用 LACP 双归。`display mlag` 应看到共享 System MAC；`display eth-trunk 1` 两个成员 Up；PC1 与 PC2 互通。断开一条上行验证业务不中断，再思考 peer-link 断开时真实网络会怎样。
