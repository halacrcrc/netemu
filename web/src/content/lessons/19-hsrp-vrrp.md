---
title: 首跳冗余的三张面孔：HSRP · VRRP · VRRP
order: 19
level: MAP
topic: fhrp
lab: campus-capstone
---

终端只配一个默认网关。网关一挂，整网断联。**首跳冗余协议（FHRP）** 让两台路由器共用一个虚拟网关 IP：主设备负责转发并应答 ARP，备设备监听心跳，主设备失联后接管。

## 三家叫什么

```
思科        HSRP（Hot Standby Router Protocol，私有）
            VRRP（标准，思科同样支持）
            GLBP（私有，网关负载分担）
华为        VRRP（标准）
华三        VRRP（标准）
```

思科生态里最常见的是 **HSRP**，华为/华三则以 **VRRP** 为主。二者目标一致，但细节不同：

```
项目             HSRP                         VRRP
角色             Active / Standby             Master / Backup
虚拟 MAC         0000.0c07.acXX               0000.5e00.01XX
默认优先级       100                          100
抢占默认         关闭                         开启
标准             思科私有                      RFC 5798
```

## 真实命令参考

```
思科 HSRP：
interface g0/0
 standby 1 ip 10.0.0.1
 standby 1 priority 200
 standby 1 preempt

华为/华三 VRRP：
interface g0/0/0
 vrrp vrid 1 virtual-ip 10.0.0.1
 vrrp vrid 1 priority 200
 vrrp vrid 1 preempt-mode
```

注意 HSRP 默认**不抢占**：主设备恢复后不会自动夺回角色，需要显式 `preempt`；VRRP 默认抢占，主设备恢复会重新成为 Master。

## 在本模拟器里

模拟器实现的是标准 **VRRP**：优先级高者当 Master，平局比接口 IP；Master 应答虚拟 IP 的 ARP；1s 通告、3.5s 失效切换。HSRP 作为思科私有协议未单独建模，但可以用 VRRP 的实验理解同一套主备逻辑。

```
vrrp 1 ip 10.0.0.1          （思科）
vrrp vrid 1 virtual-ip 10.0.0.1   （华为/华三）
```

验证：`show vrrp` / `display vrrp` 看 Master、优先级与虚拟 IP。

## 动手

打开实验九：CORE1/CORE2 共享 VRRP VIP 10.0.0.1，CORE1 优先级 200 当 Master。shutdown CORE1 的上行口，观察 CORE2 接管、终端网关不换 IP。再思考：如果两台优先级相同，谁当 Master？
