---
title: 网络运维与自动化入门
order: 36
level: IA
topic: ops
lab: campus-capstone
---

HCIA-Datacom V2.0 把**网络运维**和**自动化**单独列为模块。会配设备只是起点，能证明网络正常、快速定位故障、把重复操作交给程序，才是日常工作的主体。

## 运维的三件基本功

1. **看状态**：接口 Up/Down、IP、路由、邻居、CPU/内存；
2. **做验证**：ping 测连通、traceroute 看路径、抓包看报文；
3. **留证据**：配置备份、日志、变更记录，故障时能对比"变更前/后"。

```
ping 10.0.0.2
show ip interface brief
show ip route
show running-config
```

## 常见故障的排查顺序

- **物理层**：线缆、光模块、接口是否 Up；
- **二层**：VLAN、Trunk 放行、MAC 表、STP 状态；
- **三层**：接口 IP/掩码、网关、路由表、ARP；
- **策略层**：ACL、NAT、防火墙策略、QoS；
- **应用层**：DNS、端口、服务进程。

从下往上查，比"凭感觉改配置"快得多。每次只改一个变量，改完立刻验证并记录。

## 自动化的起点

- **数据格式**：JSON/XML/YAML；
- **接口**：SSH 批量执行、NETCONF/RESTCONF、SNMP/Telemetry；
- **工具**：Python、Ansible、控制器（iMaster NCE / DNA Center）；
- **目标**：把"配置模板 + 变量"变成可重复、可回滚的流程。

本模拟器提供 `window.__netemu.exec`，可以用脚本批量执行 CLI，体验程序化配置的第一步；完整控制器与 NETCONF 在 IE 自动化课程展开。

## 动手

打开实验九，按"物理→二层→三层→策略→应用"的顺序逐跳 ping，记录每一跳的结果；再把配置步骤写成一份可复用的清单（VLAN/网关/OSPF/NAT），思考哪些步骤可以模板化、哪些必须先验证再继续。
