---
title: 链路聚合与堆叠：把多条链路、多台设备用成一份
order: 35
level: IA
topic: lag
lab: lag-redundancy
---

园区接入常见两个需求：**带宽不够**、**单设备不可靠**。链路聚合把多条物理链路捆成一条逻辑链路；堆叠把多台交换机变成一台逻辑设备。两者都能让 STP 少阻塞、让业务多一份冗余。

## 链路聚合（LACP）

- 成员口加入同一个聚合组，通过 **LACP（802.3ad）** 协商；
- 两端都 `active` 或一端 `active` 一端 `passive` 才能选上；两端都 `passive` 不会主动发 LACPDU；
- 成员按源/目的 MAC、IP、端口哈希分担流量，单条流仍走同一条链路；
- STP 把聚合口看成一个逻辑端口，不会因为成员多而阻塞。

```
思科：
interface g0/0/1
 channel-group 1 mode active
华为：
interface g0/0/1
 eth-trunk 1
华三：
interface g0/0/1
 port link-aggregation group 1
```

本模拟器要求 VLAN/Trunk 配在**成员口**上，聚合口本身不承载二层配置。

## 堆叠（iStack / IRF / StackWise）

- 多台交换机配置同一个 **domain**，各自指定 **member** 编号；
- 优先级高者选举为 Master，成员间链路成为内部 fabric，不跑 STP；
- 对外一个管理 IP、一份配置；成员故障时其余成员继续转发；
- 风险是**分裂**：堆叠链路全断时两台都以为自己是主，真实设备需要 MAD 检测。

```
stack domain 10
stack member 1
stack priority 200
```

## 验证

```
display eth-trunk 1      show etherchannel summary
display stack            show stack
```

## 动手

打开实验五：两条互联线加入 Eth-Trunk 1，确认两个成员 Up，PC1 ping PC2 通；shutdown 一个成员，业务不中断。再打开实验十六：两台交换机加入同一 stack domain，`display stack` 看到 Master 与两个成员，PC1 跨成员互通。
