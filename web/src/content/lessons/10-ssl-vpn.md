---
title: SSL VPN：不开客户端也能远程接入
order: 10
level: IP
topic: sslvpn
lab: sslvpn-remote
---

IPsec 适合"网对网"的固定站点互联；员工在家里、在酒店要访问公司内网，更适合 **SSL VPN**：基于浏览器同款的 TLS，客户端认证通过后获得一个**虚拟 IP**，之后访问内网就像坐在办公室里一样。

## 和 IPsec 的分工

- **IPsec**：站点到站点，两台网关长期互信，保护的是网段之间的流量；
- **SSL VPN**：用户到网关，按人认证、按会话分配地址，天然适合移动办公；
- **默认拒绝仍然是前提**：外网接口在 untrust，内网在 trust，SSL VPN 是唯一被允许的入口。

## 三条配置线

1. **虚拟地址池**：给远程客户端分配的内网网段，不能和现有网段冲突；
2. **AAA 认证**：本地用户或外部认证服务器，决定"谁能连"；
3. **网关策略**：放行 SSL VPN 端口/隧道，并让虚拟 IP 能访问内网资源。

```
华为/华三：
aaa
 local-user admin password cipher Admin@123
sslvpn virtual-ip 10.10.0.0 24
思科：
username admin privilege 15 secret Admin@123
sslvpn virtual-ip 10.10.0.0 24
```

在 PC 终端里用 `sslvpn connect 203.0.113.1 admin Admin@123` 发起连接；成功后本机会拿到 10.10.0.2。`display sslvpn session` / `show sslvpn session` 能看到虚拟 IP、客户端真实 IP 与会话状态。

## 动手

打开实验十二：FW1 外网 203.0.113.1、内网 192.168.1.1，虚拟池 10.10.0.0/24。**未连接时** PC1 ping 192.168.1.10 应不通；执行 `sslvpn connect` 后再次 ping 应通。断开连接再 ping 一次，观察会话消失、访问恢复被拒——隧道是唯一的通路。
