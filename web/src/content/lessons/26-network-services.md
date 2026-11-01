---
title: 网络服务与管理：SSH、NTP、SNMP、Syslog
order: 26
level: IA
topic: services
lab: mgmt-ssh
---

网络不只是"通"，还要"可管理、可审计、可对时"。这些服务大多跑在管理面，配错不会立刻断业务，却会在故障排查时让你束手无策。

## 远程管理：Telnet 与 SSH

- **Telnet**：明文传输，口令可被抓包直接看到，只适合实验环境；
- **SSH**：加密传输，支持密钥认证，是生产环境的最低要求；
- **AAA**：认证（是谁）、授权（能做什么）、计费（做了什么）。

```
华为/华三：
aaa
 local-user admin password cipher Admin@123
user-interface vty 0 4
 authentication-mode aaa
 protocol inbound ssh
思科：
username admin privilege 15 secret Admin@123
line vty 0 4
 login local
 transport input ssh
```

## 接入安全：AAA、802.1X 与二层防护

- **AAA**：认证/授权/计费；本地用户适合小网，集中 RADIUS/TACACS+ 适合多设备；
- **802.1X**：端口级认证，未认证终端只能发 EAP/DHCP/DNS；常与动态 VLAN/ACL 联动；
- **MAC 认证**：适合打印机/IoT，配置简单但易伪造；
- **端口安全**：限制端口学习的 MAC 数，违规 `restrict` 或 `shutdown`；
- **DHCP Snooping**：只信任接 DHCP 服务器的端口，丢弃非信任口的 Offer/Ack，并建立绑定表；
- **IPSG**：依据 DHCP Snooping 绑定表检查源 IP，防止私设地址冒用。

这些能力通常组合使用：先认证，再限制 MAC/地址，最后用 ACL 控制访问。

## 时间、日志与监控

- **NTP**：所有设备对时，日志时间戳才能对齐；分层（stratum）越小越接近权威时钟；
- **Syslog**：把日志集中到服务器，按 severity（0 紧急 ~ 7 调试）过滤；
- **SNMP**：读取接口流量、CPU、温度等指标；v3 支持认证与加密，v1/v2c 的 community 相当于明文口令；
- **FTP/TFTP/HTTP**：备份配置、升级镜像；TFTP 简单无认证，只适合受控网络。

## 一条实用的管理面基线

1. 管理网独立 VLAN，ACL 限制来源；
2. 只开 SSH，关闭 Telnet；
3. 本地强口令 + 集中 AAA；
4. NTP 对时、Syslog 集中、SNMPv3；
5. 配置定期备份，变更留记录。

## 动手

打开实验六：R1 配本地用户 admin，VTY 只允许 SSH。PC1 用 `ssh 192.168.1.1` 登录成功，`telnet 192.168.1.1` 被拒绝。再思考：如果只配了 `transport input ssh` 却没配本地用户，登录会成功吗？
