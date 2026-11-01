---
title: 防火墙：安全区域与状态化策略
order: 8
level: IP
topic: security
lab: firewall-zone
---

路由器的 ACL 是"无状态"的：去程放行，回程还得再写一条。防火墙的核心升级是**安全区域 + 状态化会话**：你只需要描述"谁可以主动访问谁"，回包由会话表自动放行。

## 区域是第一道抽象

把接口按信任程度划进区域，常见三类：

- **trust**：内网，信任度最高；
- **untrust**：互联网，信任度最低；
- **dmz**：对外发布服务器，介于两者之间。

同一区域内部默认互通，**不同区域之间默认拒绝**。所以策略的本质是给区域对开洞，而不是给每个 IP 写规则。

```
华为/华三：
firewall zone trust
 add interface g0/0/0
firewall zone untrust
 add interface g0/0/1
思科：
zone security trust
zone security untrust
interface g0/0/0
 zone-member security trust
```

## 安全策略与状态会话

策略按顺序匹配，首条命中生效。放行 `trust → untrust` 后，内网发起的连接会生成一条**会话表项**；回包命中会话直接通过，不需要反向策略。反过来，`untrust → trust` 没有策略，外部主动发起的连接会被默认拒绝——这正是防火墙和"双向 ACL"的区别。

```
华为/华三：
security-policy
 rule name allow-out
  source-zone trust
  destination-zone untrust
  action permit
思科：
zone-pair security ZP
 source trust
 destination untrust
 action permit
```

验证：`display security-policy` / `show zone-pair security` 看规则，`display firewall zone` / `show zone security` 看接口归属。

## 动手

打开实验十：PC1 在 trust、SRV 在 untrust。先让 SRV 主动 ping PC1——应失败；再让 PC1 ping SRV——应成功，且 SRV 的回包靠会话表通过。把 permit 改成 deny 再试一次，体会"默认拒绝"的边界。
