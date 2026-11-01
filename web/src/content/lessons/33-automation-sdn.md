---
title: 网络自动化与 SDN：从命令行到意图
order: 33
level: IE
topic: automation
lab: campus-capstone
---

当网络从几十台设备增长到几千台，逐台 SSH 敲命令既慢又容易出错。**网络自动化**用程序、模板和 API 管理设备；**SDN** 则把控制面从设备里抽出来，交给控制器统一计算。

## 三个平面

- **数据面**：真正转发报文（ASIC/芯片）；
- **控制面**：算路由、跑协议（OSPF/BGP/STP）；
- **管理面**：配置、监控、告警（SSH/SNMP/NETCONF）。

传统设备三者合一；SDN 把控制面上收到控制器，设备变成"可编程转发器"，通过南向接口接收指令。

## 自动化工具箱

```
数据格式     JSON、XML、YAML
南向协议     NETCONF、RESTCONF、gNMI、OpenFlow
模型语言     YANG（描述配置与状态的结构）
采集         SNMP 轮询、gNMI Telemetry 流式推送
编排工具     Ansible（无 Agent）、Python、Terraform
控制器       iMaster NCE、Cisco DNA Center、OpenDaylight
```

- **NETCONF**：基于 SSH，用 XML 配置，支持 candidate/commit 回滚；
- **RESTCONF**：把 YANG 模型映射成 RESTful API，JSON 更友好；
- **Telemetry**：设备主动推送，秒级采集，替代分钟级 SNMP 轮询；
- **Ansible**：用 YAML playbook 描述"目标状态"，幂等执行，不需要在设备上装 Agent。

## 从"配置"到"意图"

自动化成熟度可以分四级：

1. **脚本化**：把命令写成脚本，逐台执行；
2. **模板化**：Jinja2/变量生成配置，减少手误；
3. **编排化**：控制器统一下发，校验与回滚；
4. **意图化**：只描述"我要什么"（如"访客网络隔离且限速"），系统自动翻译成设备配置并持续校验。

**数字孪生**（digital twin）在变更前仿真验证，配合 Telemetry 做闭环，是大型网络降低变更风险的关键。

## 在本模拟器里

模拟器聚焦逐包转发与 CLI 行为，未实现 NETCONF/YANG 控制器。但你可以用 Python 脚本调用页面暴露的 `window.__netemu.exec` 批量执行命令，体验"程序化配置"的第一步。

## 动手

打开实验九，把配置步骤写成一份"意图清单"：VLAN、网关、OSPF、NAT、ACL 各自的目标状态是什么？再思考：如果由控制器下发，哪些步骤可以并行、哪些必须串行、失败后如何回滚？这份清单就是自动化编排的输入。
