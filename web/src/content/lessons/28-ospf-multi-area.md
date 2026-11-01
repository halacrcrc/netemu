---
title: OSPF 多区域：LSA、ABR 与快速收敛
order: 28
level: IE
topic: ospf
lab: campus-capstone
---

单区域 OSPF 在大型网络里会带来两个问题：拓扑变化全区域重算、LSDB 膨胀。**多区域**把路由器分组，区域内部细算，区域之间只传汇总路由。

## 区域角色

- **骨干区域 0**：所有区域必须与它相连，区域间路由经过它中转；
- **ABR（区域边界路由器）**：同时连接区域 0 与其他区域，生成 Type-3 汇总 LSA；
- **ASBR（自治系统边界路由器）**：引入外部路由，生成 Type-5 LSA（NSSA 内为 Type-7）。

```
区域类型      特点
普通区域      接受 Type-3/4/5
Stub          拒绝 Type-4/5，用默认路由出区
Totally Stub  只保留一条默认路由，进一步精简
NSSA          允许引入外部路由（Type-7），拒绝 Type-5
```

## LSA 速查

```
Type 1  Router LSA        每台路由器在本区域内泛洪
Type 2  Network LSA       DR 生成，描述广播网段
Type 3  Summary LSA       ABR 生成，区域间网段
Type 4  ASBR Summary      ABR 告诉其他区域 ASBR 在哪
Type 5  External LSA      ASBR 引入的外部路由
Type 7  NSSA External     NSSA 内的外部路由
```

## 收敛与优化

- **SPF 节流**：限制 SPF 计算频率，避免抖动导致 CPU 飙升；
- **PRC（部分路由计算）**：只有叶节点变化时不算整棵树；
- **BFD**：毫秒级检测邻居故障，比 Hello 超时快得多；
- **LSA 汇总**：ABR 用 `area range` 把明细聚合成一条，缩小 LSDB；
- **Stub/NSSA**：减少不必要的外部 LSA 泛洪。

## 验证

```
display ospf peer          show ip ospf neighbor
display ospf lsdb          show ip ospf database
display ospf routing       show ip ospf route
```

排错先看邻居状态（Down/Init/2-Way/ExStart/Exchange/Loading/Full），再看 LSDB 是否一致，最后看路由表是否选对。

## 动手

打开实验九：CORE1/CORE2 在区域 0 建立邻居并互告网段。尝试把其中一台改到区域 1，观察邻居是否还能建立、路由是否仍可达；再在 ABR 上做区域汇总，对比汇总前后的 LSDB 与路由表。
