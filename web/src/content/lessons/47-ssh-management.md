---
title: SSH 远程管理：VTY、本地用户与传输
order: 47
level: IA
topic: mgmt
lab: mgmt-ssh
---

真实网络里，跑到设备跟前接 Console 线不现实。远程管理走 VTY（虚拟终端）线路，而 **SSH 是生产环境的最低要求**——Telnet 全程明文，口令抓包即得。

## SSH 登录要过三道门

1. **VTY 线路放行**：`line vty 0 4`（华为 `user-interface vty 0 4`）下用 `transport input ssh` / `protocol inbound ssh` 限制只允许 SSH，Telnet 直接被拒。
2. **认证凭据**：`login local`（IOS）或 `authentication-mode aaa`（华为/华三）——让 VTY 用本地用户表或 AAA 验证，而不是不验证。
3. **服务端用户**：`username admin secret cisco123`（IOS）/ `aaa` + `local-user admin`（华为/华三）——**认证发生在服务器端**，没建用户就没人能登录。

## 经典坑：配了用户还是登不上

SSH 需要设备生成加密密钥。密钥没生成时，用户名密码都对也登不上。排障顺序：

```
1. show ip ssh / display ssh server status   ← 服务启用了吗？
2. 检查 VTY 的 transport / protocol inbound   ← 放行 SSH 了吗？
3. 检查本地用户是否存在                        ← 有凭据可验证吗？
```

> 模拟器注记：`show ip ssh` / `display ssh server status` 暂未实现（SSH 服务默认开启、无需生成密钥）。在模拟器里验证前两步可以直接发起 `ssh` 登录：连接被拒查 VTY 传输，`Permission denied` 查用户与口令。真实设备上密钥未生成（IOS 缺 `crypto key generate rsa`）是"配了用户仍登不上"的头号原因。

## SSH 会话的行为

登录成功后，提示符切换为**远端设备的 CLI**，后续每条命令都在远端执行——`exit` 关闭会话回到本地。这与 Console 的区别只是传输路径，操作体验一致。

## 动手

打开实验「远程管理（SSH/Telnet）」：给路由器建本地用户、限制 VTY 只走 SSH，再从对端设备 `ssh` 登录进来；抓包对比 Telnet 与 SSH 的报文内容。
