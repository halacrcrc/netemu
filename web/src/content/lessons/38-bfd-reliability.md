---
title: BFD 与网络可靠性：把故障检测做到毫秒级
order: 38
level: IP
topic: fhrp
lab: campus-capstone
---

路由协议靠 Hello 检测邻居，OSPF 死亡时间默认 40 秒、BGP 更久。链路故障后业务要等几十秒才切换——**BFD** 用轻量心跳把检测压到毫秒级，再联动路由/网关协议快速倒换。

## BFD 怎么工作

- 两端建立 BFD 会话，周期发送检测报文（可到 3.3ms 级）；
- 连续丢失约定数量后判定 Down，通知关联协议；
- **异步模式**：周期发检测包；**回声模式**：一端回环检测，减少对端处理；
- 常与 **OSPF/BGP/VRRP/静态路由** 联动：BFD 一 Down，协议立即收敛或主备切换。

## 可靠性的四层手段

1. **链路级**：链路聚合（LACP）、双上行；
2. **设备级**：堆叠/IRF、M-LAG/vPC/DRNI；
3. **网关级**：VRRP/HSRP 主备；
4. **路径级**：IGP 快速收敛、BGP 多路径、FRR/TI-LFA。

BFD 不替代这些机制，而是让它们**更快知道故障发生**。例如 VRRP 默认 3.5 秒切换，联动 BFD 后可降到百毫秒级。

## 验证思路

```
display bfd session all        show bfd neighbors
display vrrp                  show vrrp
display ospf peer             show ip ospf neighbor
```

排错时先确认 BFD 会话是否 Up，再看关联协议是否收到 Down 通知，最后确认备份路径是否真的可用。

## 在本模拟器里

模拟器实现了 VRRP 主备与链路聚合，但未建模 BFD 报文。可以打开实验九观察 VRRP 切换时间，再思考：如果加入 BFD，切换会快多少？哪些场景（如单链路抖动）反而不适合把 BFD 调得过快？
