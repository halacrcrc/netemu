---
title: 广域互联与承载：SD-WAN、SRv6 与 CloudWAN
order: 41
level: IE
topic: mpls
lab: mpls-l3vpn
---

HCIE-Datacom V2.0 的广域网络模块占 33%。企业分支不再只靠专线：**SD-WAN** 用 Overlay 把 Internet/MPLS/5G 等多种 Underlay 统一编排；运营商侧则用 **SRv6/CloudWAN** 做可编程承载。

## SD-WAN 的三层

- **Underlay**：物理链路（MPLS、Internet、LTE/5G），负责可达；
- **Overlay**：GRE/IPsec 隧道，负责加密与拓扑解耦；
- **控制器**：集中下发策略，按应用质量选路（A-FEC、链路切换）。

关键能力：零配置上线（ZTP）、应用识别、按需选路、集中策略、可视运维。

## SRv6 与 CloudWAN

- **SRv6** 把路径编码为 IPv6 段列表（Segment List），源端决定经过哪些节点；
- **BE（Best Effort）** 与 **Policy** 两种模式；Policy 可做显式路径与流量工程；
- **网络切片**：不同业务用不同 Segment List/资源，互不抢占；
- **TI-LFA**：拓扑无关的无环备份，故障亚秒切换；
- **CloudWAN**：把 SRv6、EVPN、控制器与云网协同组合成承载方案。

## 与 MPLS L3VPN 的关系

MPLS L3VPN 仍是企业专线主流：PE 用 VRF 隔离、MP-BGP 传 VPNv4、标签转发。跨域有 Option A/B/C。SRv6 可以替代部分 LSP 信令，简化承载；两者常共存演进。

## 设计关注点

- 分支数量与链路质量差异，是否需要应用级选路？
- Overlay 加密用 IPsec/GRE，密钥与证书如何管理？
- 关键业务是否需要 SRv6 Policy/切片保障？
- 控制器故障时，数据面能否继续转发？
- 与现有 MPLS VPN 如何互通与迁移？

## 动手

打开实验八的 MPLS L3VPN 拓扑，先理解"VRF + 双标签"的专线模型；再设想把 P 网络换成 SRv6 承载：入口 PE 压 Segment List，中间节点按 SID 转发。画出两种方案的标签/段列表差异，就是 SD-WAN/CloudWAN 演进的切入点。
