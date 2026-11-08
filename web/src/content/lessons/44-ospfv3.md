---
title: OSPFv3：IPv6 世界的链路状态路由
order: 44
level: IP
topic: ospfv3
lab: ospfv3
---

OSPFv3 是 OSPF 面向 IPv6 的版本。邻居、区域、SPF 的思想不变，但地址、报文和部分机制换了：用链路本地地址建邻居、用 Link-LSA 描述链路、Router ID 仍是 32 位 IPv4 格式。

## 与 OSPFv2 的关键差异

- **按链路而不是按网段**：OSPFv3 在接口/链路上运行，同一接口可有多个 IPv6 前缀；
- **链路本地地址**：邻居通常用 `fe80::/10` 建邻接，下一跳也是链路本地；
- **Link-LSA**：传递接口的链路本地地址与前缀，SPF 仍按 Router-LSA 计算；
- **认证**：OSPFv3 头部不再带认证字段，改用 IPv6 IPsec；
- **Router ID**：仍是 32 位，必须手工指定或选一个 IPv4 地址。

## 配置

```
华为/华三：
ospfv3 1
 router-id 1.1.1.1
interface g0/0/1
 ipv6 address 2001:db8:12::1/64
 ospfv3 1 area 0
思科：
ipv6 router ospf 1
 router-id 1.1.1.1
interface g0/0
 ipv6 address 2001:db8:12::1/64
 ipv6 ospf 1 area 0
```

验证：`display ospfv3 peer` / `show ipv6 ospf neighbor`、`display ipv6 routing-table` / `show ipv6 route`。路由表里 OSPFv3 路由标记为 `O`。

## 本模拟器的简化

模拟器实现了 OSPFv3 的**邻居建立与前缀学习**：接口启用后周期发送前缀列表，邻居学习并安装 IPv6 路由；不做完整 LSA 泛洪、DR 选举与区域间汇总。对于“两台/多台直连路由器自动学习对端网段”的教学目标足够，复杂拓扑仍需真实设备验证。

## 动手

打开实验二十一：R1/R2/R3 三台链式配置 IPv6 地址与 OSPFv3。`display ospfv3 peer` 中 R2 应有两个邻居；`display ipv6 routing-table` 里 R3 出现它从未直连的 2001:db8:1::/64（LSA 泛洪穿越了 R2），PC1 能 ping6 PC2。再把 R2 的 `ospfv3 1 area 0` 删掉，观察泛洪如何中断、R1/R3 如何互相失联。
