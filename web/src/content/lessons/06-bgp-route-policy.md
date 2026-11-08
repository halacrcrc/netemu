---
title: BGP 与 route-policy：域间路由与策略
order: 6
level: IP
topic: bgp
lab: bgp-route-policy
---

OSPF 解决的是"一个自治系统内部怎么走"，而 **BGP** 解决的是"不同自治系统（AS）之间怎么走"。BGP 不关心链路带宽，它交换的是**路径向量**：一条路由经过哪些 AS，靠 AS_PATH、LOCAL_PREF 等属性选路。

## eBGP 的最小闭环

两台路由器分属不同 AS，互为邻居后宣告自己的网段：

```
华为/华三：
bgp 65001
 peer 10.0.0.2 as-number 65002
 network 192.168.1.0 255.255.255.0
思科：
router bgp 65001
 neighbor 10.0.0.2 remote-as 65002
 network 192.168.1.0 mask 255.255.255.0
```

注意 `network` 只是"允许宣告"，本机路由表里必须真的存在这条路由（直连或静态），BGP 才会把它发出去。

## route-policy：给路由装一道闸门

策略由若干 **node** 组成，按序号从小到大匹配：`if-match` 命中就执行 `apply`，然后离开；没命中继续下一个 node。**所有 node 都没命中就是隐式拒绝**，所以过滤型策略末尾通常补一个空的 permit node。

```
华为/华三：
acl 2000
 rule permit source 172.16.9.0 0.0.0.255
route-policy FILTER deny node 10
 if-match acl 2000
route-policy FILTER permit node 20
bgp 65002
 peer 10.0.0.1 route-policy FILTER import
思科：
access-list 2000 permit 172.16.9.0 0.0.0.255
route-map FILTER deny 10
 match ip address 2000
route-map FILTER permit 20
router bgp 65002
 neighbor 10.0.0.1 route-map FILTER in
```

`import`（思科 `in`）在**收路由时**过滤，`export`（思科 `out`）在**发路由时**过滤——方向搞反是实验里最常见的错误。

## 动手

打开实验七：R1—R2—R3 三 AS 链式，R2 用 ACL 精确匹配 172.16.9.0/24，再用 route-policy 在 import 方向拒绝它。`display bgp routing-table` / `show ip bgp` 里 R2 和 R3 都应只有 192.168.1.0/24 而没有受控网段——策略不只影响 R2 自己，还沿 BGP 传递到了下游 AS；PC2 ping 受控服务器失败，而 ping PC1 正常。
