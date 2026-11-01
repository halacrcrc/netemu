---
title: IPsec：把分支机构的私网搬到公网上
order: 9
level: IP
topic: ipsec
lab: ipsec-site
---

两个分支机构各有自己的私网，中间只能走公网。**IPsec 站点到站点 VPN** 在两条公网出口之间建一条加密隧道，把 192.168.1.0/24 与 192.168.2.0/24 的流量整体"装进"隧道里，公网只看到两个出口 IP 在通信。

## 三个关键要素

- **对端地址**：隧道的两端公网 IP，彼此必须可达；
- **保护子网**：哪些源/目的网段走隧道（也叫"感兴趣流"），没匹配的照常走公网；
- **安全参数**：加密算法、认证方式、密钥——真实设备由 IKE 自动协商，本模拟器简化为静态隧道，只校验两端子网与端点。

真实 IPsec 的流程可以记成两步：IKE 先在不安全的网络上协商出一条安全通道（阶段一），再用它协商 ESP 参数（阶段 2），之后数据报文用 ESP 封装传输。隧道模式下，原始 IP 报文整体被加密，再套一个新的公网 IP 头。

```
华为/华三：
ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2
 local-subnet 192.168.1.0 24
 remote-subnet 192.168.2.0 24
思科：
ipsec tunnel T1 local 10.0.0.1 remote 10.0.0.2
 local-subnet 192.168.1.0 24
 remote-subnet 192.168.2.0 24
```

两端要**互为镜像**：R1 的 local 是 R2 的 remote，R1 的 local-subnet 是 R2 的 remote-subnet。只配一端，隧道起不来。

## 动手

打开实验十一：R1/R2 分别配置 T1，两端子网互为镜像。`display ipsec tunnel` / `show ipsec tunnel` 应看到保护子网；PC1 ping 192.168.2.10 应通。抓包选中 R1–R2 链路，能看到外层是 10.0.0.x 的 IPsec 报文，内层才是 192.168.1.x 的原始 ICMP。
