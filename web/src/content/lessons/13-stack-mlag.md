---
title: 堆叠与 M-LAG：把两台设备用成一台
order: 13
level: IE
topic: ha
lab: mlag-dc
---

接入交换机双上行到两台汇聚，传统做法靠 STP 阻塞一条链路——带宽浪费一半，故障还要等收敛。**堆叠**和 **M-LAG** 从两个方向解决同一个问题：让两台物理设备对外表现得像一台。

## 堆叠 / IRF：真·一台逻辑设备

多台交换机配置同一个 **domain**，各自指定 **member** 编号，并用专用链路（fabric）互联。选举出的 Master 统一管理，成员间转发表同步；对外只有一个管理 IP、一份配置。

- 成员间的链路被视为**设备内部总线**，不跑 STP，也不会被阻塞；
- Master 优先级高者当选，成员编号冲突会导致加入失败；
- 升级、重启按成员滚动进行，业务不中断。

```
SW1：
stack domain 10
stack member 1
stack priority 200
SW2：
stack domain 10
stack member 2
stack priority 100
```

## M-LAG：逻辑上聚合，物理上独立

M-LAG 不合并控制面：两台 DSW 各自独立运行，但共享同一个**系统 MAC**，并通过 **peer-link** 同步表项。接入交换机用 LACP 双归到两台 DSW，两条链路同时转发，对端看到的是"同一个 LACP 邻居"。

- 共享系统 MAC 让 LACP 认为对端是一台设备；
- peer-link 承载跨设备流量与同步，必须足够可靠；
- 广播/未知单播从 peer-link 进来后，不再回送跨设备 LAG（水平分割），避免环路。

```
DSW1/DSW2：
mlag domain 1
interface g0/0/2
 mlag peer-link
```

## 动手

打开实验十六：两台交换机加入同一 stack domain，`display stack` 应看到 Master 与两个成员，PC1 跨成员 ping 通。再打开实验十七：两台 DSW 配 `mlag domain 1`，SW1 用 LACP 双归，`display mlag` 看到共享 System MAC，`display eth-trunk 1` 两个成员均为 Up，PC1 与 PC2 互通。断开其中一条上行，业务应不中断。
