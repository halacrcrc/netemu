---
title: OSPF 与 VRRP：动态路由与网关冗余
order: 5
level: IP
topic: ospf
lab: campus-capstone
---

静态路由在小型网络够用，但链路一变就要手工改。**OSPF** 让路由器互相发现、交换链路状态，自动算出无环的最短路径；**VRRP** 则让两台网关共用一个虚拟 IP，主网关故障时备用设备接管，终端无感。

## OSPF 的收敛过程

- 每台路由器从接口发 **Hello** 包（默认 10 秒）发现邻居；
- 邻居建立后交换 **LSA**（链路状态通告），拼出全网的拓扑数据库；
- 用 SPF（Dijkstra）算法算出到每个网段的最短路径，写入路由表；
- 拓扑变化时只重算受影响的部分，收敛速度远快于"等路由老化"。

多区域设计中，**区域 0 是骨干区域**，其他区域必须与它相连。宣告网段用的反掩码（wildcard）和 ACL 一样是"反着写"：`0.0.0.3` 匹配一个 /30。

```
华为/华三：
ospf 1
 area 0
  network 10.1.0.0 0.0.0.3
思科：
router ospf 1
 network 10.1.0.0 0.0.0.3 area 0
```

## VRRP：给网关一个"永不宕机"的 IP

两台路由器各配一个真实 IP，再共同持有一个**虚拟 IP（VIP）**。优先级高者成为 Master，负责应答 ARP 与转发；Backup 持续收 Master 的通告，超时后按优先级接替。开启 **preempt（抢占）** 后，原 Master 恢复会重新夺回角色。

```
华为/华三：
vrrp vrid 1 virtual-ip 10.0.0.1
vrrp vrid 1 priority 200
vrrp vrid 1 preempt-mode
思科：
vrrp 1 ip 10.0.0.1
vrrp 1 priority 200
vrrp 1 preempt
```

验证命令：`display ospf peer` / `show ip ospf neighbor` 看邻居是否 FULL；`display vrrp` / `show vrrp` 看 Master 与优先级。

## 动手

打开实验九「大型园区综合实验」：CORE1/CORE2 跑 OSPF 学习互联路由，同时用 VRRP 共享 10.0.0.1 作为 DSW 的默认网关。试着 shutdown CORE1 的上行口，观察 VRRP 切换与 OSPF 重收敛后业务是否恢复。
