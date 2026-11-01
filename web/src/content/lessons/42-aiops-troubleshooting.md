---
title: 网络智能运维与结构化排障
order: 42
level: IE
topic: ops
lab: campus-capstone
---

HCIE-Datacom V2.0 把**网络智能运维**单列（笔试 14%），实验考试还新增了故障排查模块。会用协议不够，还要能证明网络健康、在变更前后快速定位问题。

## 从轮询到流式

- **SNMP**：分钟级轮询，适合趋势与告警；
- **Telemetry（gNMI）**：设备主动推送，秒级指标与流数据；
- **Syslog/NetFlow/sFlow**：日志与流量采样，定位"谁在占带宽、哪个会话异常"；
- **AIOps**：基线学习、异常检测、根因关联，把"告警风暴"收敛成少数可行动事件。

## 数字孪生与变更

变更前在**数字孪生**里仿真：配置下发后哪些邻居会断、哪些路由会变；变更中按步骤执行并实时校验；变更后对比前后指标。无法仿真时，至少准备**回滚配置**与维护窗口。

## 结构化排障

1. **界定范围**：单用户/单网段/全网？何时开始？变更前是否正常？
2. **分层验证**：物理 → 二层 → 三层 → 策略 → 应用；
3. **对照证据**：路由表、ARP/MAC、会话表、抓包、日志时间线；
4. **假设与验证**：一次只改一个变量，改完立即验证；
5. **恢复与复盘**：先恢复业务，再找根因，最后补监控与自动化检查。

常见"假故障"：DNS 解析失败、MTU 不一致、非对称路由、NAT 会话老化、防火墙状态会话超时。

## 工具与命令

```
display interface brief     show ip interface brief
display ip routing-table    show ip route
display ospf peer           show ip ospf neighbor
display mac-address         show mac address-table
```

抓包时同时看**控制面**（BPDU/OSPF/BGP）与**数据面**（ICMP/TCP），能快速区分"协议没收敛"还是"转发被策略挡住"。

## 动手

打开实验九，人为制造三个故障：Trunk 少放行一个 VLAN、OSPF 反掩码写错、NAT 只配 inside。按上面的五步法逐一排查，记录每一步的证据与结论。最后把三个故障的检查项写成一份排障清单。
