---
title: IS-IS 基础：NET、Level 与 DIS
order: 37
level: IP
topic: isis
lab: campus-capstone
---

IS-IS 是 HCIP 核心考纲里的第二种链路状态协议。它和 OSPF 一样用 SPF 算路，但报文直接封装在二层，扩展性更强；运营商与大型数据中心至今仍大量使用。

## 三个必会概念

- **NET**：网络实体名，例如 `49.0001.0000.0000.0001.00`；区域地址 + System ID + SEL，System ID 唯一标识路由器；
- **Level 1 / Level 2**：L1 负责区域内部，L2 构成骨干；L1-2 路由器同时属于两层，类似 OSPF 的 ABR；
- **DIS**：广播网段上的指定中间系统，类似 DR，但**没有备份 DIS**，且可被更高优先级的路由器抢占。

IS-IS 用 **LSP** 描述链路状态，用 CSNP/PSNP 同步数据库；TLV 结构让它天然支持 IPv6、SR、TE 等扩展。

## 配置

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

注意思科接口命令是 `ip router isis`；华为/华三是 `isis enable`。

## 与 OSPF 的对比

- **区域边界**：OSPF 在接口上划分区域，IS-IS 在路由器级别划分 L1/L2；
- **指定设备**：OSPF 有 DR/BDR，IS-IS 只有 DIS 且无备份；
- **封装**：OSPF 跑在 IP 之上（协议号 89），IS-IS 直接跑在二层；
- **扩展**：IS-IS 的 TLV 更灵活，是 SR/SRv6 的常用承载。

验证：`display isis peer` / `show isis neighbors`、`display isis lsdb` / `show isis database`。

## 动手

打开实验九的核心链路，把 OSPF 思路平移过来：两台核心配置同一区域地址、不同 System ID，接口启用 IS-IS，观察 L1 邻居与路由。再思考：如果一台只配 L2，另一台是 L1-2，邻居关系会怎样？
