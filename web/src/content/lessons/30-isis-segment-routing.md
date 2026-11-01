---
title: IS-IS 与 Segment Routing
order: 30
level: IE
topic: isis
lab: campus-capstone
---

IS-IS 是运营商与大型数据中心常用的链路状态协议，和 OSPF 思路相似但报文基于 CLNS；**Segment Routing（SR）** 则把路径编码进标签或 IPv6 地址，让转发面不再依赖逐跳协议状态。

## IS-IS 的三个关键词

- **NET**：网络实体名，格式如 `49.0001.0000.0000.0001.00`，其中 System ID 唯一标识路由器；
- **Level 1 / Level 2**：L1 区域内、L2 骨干；L1-2 路由器同时属于两层，类似 OSPF 的 ABR；
- **DIS**：广播网段上的指定中间系统，类似 DR，但**没有备份 DIS**，且默认 10 秒周期选举。

IS-IS 报文直接封装在二层（不是 IP），扩展性强，TLV 可以承载 IPv6、SR、TE 等能力。它用 **LSP** 描述链路状态，用 CSNP/PSNP 同步数据库。

## Segment Routing 的核心

传统 MPLS 依赖 LDP/RSVP 逐跳建 LSP；SR 则让**源节点**把路径写成标签栈（SR-MPLS）或 IPv6 段列表（SRv6）：

- **Node SID / Prefix SID**：给每台路由器/前缀分配一个段标识；
- **Adjacency SID**：标识某条邻接链路；
- **SRGB**：全局标签块，保证标签在域内唯一；
- **TI-LFA**：拓扑无关的无环备份，故障后亚秒级切换。

```
SR-MPLS 转发：
入口压入 [Node-SID(R2), Node-SID(R4)]
R2 弹出自己的 SID，按栈顶转发
R4 收到后弹出，交付目的网段
```

SRv6 把 SID 写成 IPv6 地址（Locator + Function），不再需要 MPLS 标签，天然支持 SDN 编程与业务链。

## 配置与验证

```
华为/华三：
isis 1
 network-entity 49.0001.0000.0000.0001.00
interface g0/0/1
 isis enable
思科：
router isis
 net 49.0001.0000.0000.0001.00
interface g0/0
 ip router isis
```

验证：`display isis peer` / `show isis neighbors`、`display isis lsdb` / `show isis database`。

## 动手

打开实验九的核心链路，把 OSPF 换成 IS-IS 的思路做一次推演：两台核心同区域建立 L1 邻居，再引入外部路由。真实 SR 部署还需要控制器与标签规划，本模拟器只覆盖 IS-IS 邻居与路由学习。
