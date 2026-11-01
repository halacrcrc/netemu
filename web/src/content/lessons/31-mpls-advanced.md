---
title: MPLS VPN 跨域与流量工程
order: 31
level: IE
topic: mpls
lab: mpls-l3vpn
---

单域 L3VPN 的模型是"PE 压两层标签、P 换外层、出口 PE 按 VPN 标签投递"。真实运营商网络往往跨多个 AS，还要按带宽与延迟选路——这就是跨域 VPN 与 MPLS TE 要解决的问题。

## 跨域 L3VPN 三种方案

- **Option A（背靠背 VRF）**：ASBR 之间用 VRF 子接口/VRF 实例对接，逐跳重建 VPN；简单但扩展性差；
- **Option B（MP-eBGP）**：ASBR 之间跑 MP-BGP 交换 VPNv4 路由，报文在 ASBR 上换标签；中等规模常用；
- **Option C（多跳 MP-eBGP）**：PE 之间直接交换 VPNv4，ASBR 只转发带标签的报文；扩展性最好，配置最复杂。

关键区别在于：**VPN 路由在哪里交换、标签在哪一层重新分配**。

## RD 与 RT

- **RD（Route Distinguisher）**：8 字节，拼在 IPv4 前缀前，让不同 VRF 的相同网段在 MP-BGP 中唯一；
- **RT（Route Target）**：扩展社区，决定路由"被导出到哪些 VRF / 从哪些 VRF 导入"；
- RD 负责唯一性，RT 负责导入导出策略，两者不能混用。

## MPLS TE：按约束选路

传统 IGP 只按度量选最短路径，不管带宽。**TE** 先算满足带宽/亲和属性的路径（CSPF），再用 **RSVP-TE** 建立带带宽预留的 LSP：

- **显式路径**：手工指定经过的节点/链路；
- **FRR**：预先建立备份 LSP，故障时快速切换；
- **亲和属性**：避开或倾向某些链路（如高延迟链路）。

**SR-TE** 则用 Segment List 表达路径，不再需要 RSVP 逐跳信令，和 SR 一起成为新一代承载网主流。

## 验证与排错

```
display mpls lsp            show mpls lsp
display bgp vpnv4 all routing-table
display ip routing-table vpn-instance CUST1
```

排错顺序：VRF 接口绑定 → RD/RT 导入导出 → MP-BGP 邻居 → LSP 标签栈 → 回程 LSP。跨域场景尤其要确认**回程标签**是否在另一侧正确分配。

## 动手

打开实验八：先跑通单域静态 LSP，再把 P 路由器想象成两个 ASBR，推演 Option B 的标签变化：入口压传输+VPN 标签，ASBR 换传输标签并保留 VPN 标签，出口 PE 按 VPN 标签投递。把每一步的标签栈画在纸上，比背命令更有效。
