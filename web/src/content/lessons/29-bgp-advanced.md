---
title: BGP 进阶：选路属性与路由反射器
order: 29
level: IE
topic: bgp
lab: bgp-route-policy
---

BGP 交换的是"可达性 + 路径属性"。同一个目的前缀可能从多个邻居学到，路由器按一套固定的比较顺序选出最优路径；网络规模变大后，还要解决 iBGP 全互联的扩展性问题。

## 选路顺序（简化）

```
1. Weight（思科私有，越大越优）
2. LOCAL_PREF（越大越优，AS 内统一出口策略）
3. 本地始发（network/aggregate）优先
4. AS_PATH 最短
5. Origin（IGP < EGP < Incomplete）
6. MED（越小越优，影响入 AS 流量）
7. eBGP 优于 iBGP
8. 到 NEXT_HOP 的 IGP 度量最小
9.  router ID 最小 / 邻居地址最小（打破平局）
```

记忆口诀：**先本地偏好，再路径长短，再看来源与 MED，最后比下一跳**。实际排错时先确认"到底走到了哪一步"。

## iBGP 全互联与路由反射器

iBGP 有水平分割：从 iBGP 邻居学到的路由不再传给其他 iBGP 邻居。AS 内 N 台路由器需要 N(N−1)/2 条会话，规模一大就不可维护。**路由反射器（RR）** 允许把路由反射给 client，打破水平分割；RR 与 client 组成 cluster，用 cluster ID 防环。**联盟（Confederation）** 则是把一个大 AS 切成若干子 AS，减少会话数量。

## 常用属性与策略

- **LOCAL_PREF**：AS 内选出口，常用 route-policy 在 import 时设置；
- **MED**：向邻居建议入口，影响对方如何进来；
- **Community**：给路由打标签（如 no-export、no-advertise），配合策略批量处理；
- **Next-Hop**：iBGP 默认不改变下一跳，需确保 IGP 可达或用 `next-hop-self`。

```
route-policy SET-LP permit node 10
 apply local-preference 200
bgp 65001
 peer 10.0.0.2 route-policy SET-LP import
```

## 稳定性

- **BFD** 快速检测邻居；
- **路由抖动抑制** 防止翻动路由反复占用 CPU；
- **最大前缀数** 防止邻居误宣告大量路由压垮设备；
- **GTSM/TTL 安全** 防止伪造 BGP 报文。

## 动手

打开实验七：观察 R2 如何用 route-policy 过滤前缀。再思考：如果 R2 同时从两个邻居学到 192.168.1.0/24，一个 LOCAL_PREF=200、一个 AS_PATH 更短，最终选哪条？把答案写进笔记，并用 `display bgp routing-table` 验证。
