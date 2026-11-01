---
title: 大型园区综合：多协议协同与排障
order: 15
level: IE
topic: campus
lab: campus-capstone
---

真实园区不会只考一个协议。接入层划 VLAN、汇聚层做网关、核心层跑路由与出口 NAT，任何一层配错都表现为"某台 PC ping 不通"。这一课把前面学过的技术串成一条**自下而上的排障路径**。

## 分层看拓扑

- **接入层（SW1/SW2）**：Access 口接终端，Trunk 上行，双链路 LACP 聚合；
- **汇聚层（DSW）**：VLAN 间路由（SVI），默认网关指向核心的 VRRP VIP；
- **核心层（CORE1/CORE2）**：OSPF 互联、VRRP 冗余、出口 NAT；
- **出口**：公网服务器只看到 NAT 后的地址。

## 排障顺序：从物理到应用

1. **链路与端口**：`display interface brief` 看 Up/Down、VLAN 归属；
2. **二层**：`display vlan`、`display eth-trunk 1` 确认 Trunk 放行与成员选中；
3. **三层**：`display ip routing-table` 看有没有目标网段，`display ospf peer` 看邻居；
4. **网关冗余**：`display vrrp` 看 Master 与 VIP；
5. **出口**：`display nat session` 看转换是否发生；
6. **端到端**：从 PC 一路 ping 网关、核心、公网，逐跳缩小范围。

常见坑：Trunk 忘放行 VLAN、聚合成员口配了 Trunk 但逻辑口没配（本模拟器聚合口不承载二层配置）、OSPF 宣告了错误的反掩码、VRRP 优先级相同导致 Master 不稳定、NAT 只配了 inside 没配 outside。

## 关键配置片段

```
DSW：
vlan batch 10 20 99
interface vlanif 10
 ip address 192.168.10.1 24
ip route-static 0.0.0.0 0 10.0.0.1
CORE1：
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
interface g0/0
 vrrp 1 ip 10.0.0.1
 vrrp 1 priority 200
 vrrp 1 preempt
```

## 动手

打开实验九：按任务完成 VLAN、LACP、SVI、VRRP、OSPF 与 NAT。依次验证 PC1 → PC2（同 VLAN 跨楼）、PC1 → PC3（跨 VLAN）、PC1 → 203.0.113.10（NAT 出口）。任何一步失败，就按上面的六层顺序从下往上查——这比"凭感觉改配置"快得多。
