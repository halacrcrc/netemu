---
title: OSPF 单区域：邻居、DR 与最短路径
order: 34
level: IA
topic: ospf
lab: campus-capstone
---

HCIA 的 IP 网络模块里，**OSPF 单区域**是静态路由之后的第一种动态路由。它让路由器自动发现邻居、交换链路状态，并用 SPF 算法算出无环最短路径。

## 邻居是怎么建立的

- 路由器从接口周期发 **Hello**（默认 10 秒，死亡 40 秒）；
- Hello 里携带 Router ID、Area ID、Hello/Dead 定时器、认证与掩码；
- 参数不一致就无法建立邻居；状态从 Down → Init → 2-Way → ExStart → Exchange → Loading → **Full**；
- 广播网段选举 **DR/BDR**：优先级高者优先，平局比 Router ID；DR 负责收集并泛洪 LSA，减少邻接数量。

OSPF 用组播 `224.0.0.5`（AllSPFRouters）和 `224.0.0.6`（AllDRouters）通信。

## 单区域怎么配

所有接口都放进区域 0，网段用反掩码宣告：

```
华为/华三：
ospf 1
 area 0
  network 10.1.0.0 0.0.0.3
思科：
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
```

- **反掩码** 0 表示必须匹配，1 表示忽略；`0.0.0.3` 匹配 /30；
- **cost** 默认按接口带宽计算，可用 `ospf cost` 手工调整；
- 只宣告需要参与 OSPF 的网段，避免把管理网、外网接口卷进来。

## 验证与常见问题

```
display ospf peer          show ip ospf neighbor
display ip routing-table   show ip route
```

邻居卡在 Init/2-Way 常见于 Hello 参数不一致或 DR 选举；邻居 Full 但没有路由，先查宣告网段与反掩码，再看是否被更长前缀或更低优先级路由覆盖。

## 动手

打开实验九：CORE1/CORE2 在区域 0 建立邻居，互相学习 10.1.0.0/30 与业务网段。先看 `display ospf peer` 的 Full 状态，再看路由表里 `O` 路由；把其中一端的反掩码改错，观察邻居与路由如何变化。
