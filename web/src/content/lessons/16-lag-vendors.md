---
title: 链路聚合的三张面孔：EtherChannel / Eth-Trunk / Bridge-Aggregation
order: 16
level: MAP
topic: lag
lab: lag-redundancy
---

两台交换机之间拉多条线，如果不做聚合，STP 只会保留一条，其余全部阻塞。**链路聚合**把多条物理链路捆成一条逻辑链路：带宽叠加、成员互为备份，STP 也只看到一个端口。

## 三家叫什么

```
技术            思科                      华为                      华三
链路聚合        EtherChannel              Eth-Trunk                 Bridge-Aggregation
成员加入        channel-group 1 mode ...  eth-trunk 1               port link-aggregation group 1
LACP 模式       active / passive          lacp-static / manual      dynamic / static
查看            show etherchannel summary display eth-trunk 1        display link-aggregation verbose
```

思科还有私有的 **PAgP**（port-channel 协议），华为/华三只支持标准 **LACP（802.3ad）** 或手工模式。跨厂商对接时一定选 LACP：`active` 对 `passive`、或两端都 `active` 才能协商成功；两端都 `passive` 不会主动发 LACPDU，聚合起不来。

## 关键概念

- **活动/备份成员**：LACP 选出可转发的成员，其余待命；成员故障时自动切换；
- **负载分担**：按源/目的 MAC、IP、端口哈希把流量分散到成员，单条流仍走同一条链路；
- **STP 视角**：聚合口是一个逻辑端口，不会因为成员多而被阻塞；
- **一致性**：成员口的速率、双工、VLAN 配置应一致，否则可能只有部分成员被选中。

## 在本模拟器里

模拟器用统一的 LACP 模型：成员口加入聚合组后发 LACPDU，双方 `Selected` 才转发；逻辑口参与 STP。注意本模拟器的 VLAN/Trunk 配置要下在**成员口**上，聚合口本身不承载二层配置。

```
思科：
interface g0/0/1
 channel-group 1 mode active
华为/华三：
interface g0/0/1
 eth-trunk 1
华三：
interface g0/0/1
 port link-aggregation group 1
```

## 动手

打开实验五：把 SW1/SW2 的两条互联线加入 Eth-Trunk 1，`display eth-trunk 1` 应看到两个成员 Up；PC1 ping PC2 通。再 shutdown 一个成员口，业务应不中断——这就是聚合相对 STP 的价值。
