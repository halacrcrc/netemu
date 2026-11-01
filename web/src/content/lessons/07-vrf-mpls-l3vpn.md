---
title: VRF 与 MPLS L3VPN：一张物理网，多张虚拟网
order: 7
level: IP
topic: mpls
lab: mpls-l3vpn
---

两家客户都租用同一个运营商骨干，却要求"彼此看不见、地址可以重叠"。答案分两层：**VRF** 在设备内部把路由表和接口隔离成多份；**MPLS** 在骨干上用标签转发，让 PE 之间的报文不必关心里面装的是哪个客户的 IP。

## VRF：一台路由器里的多张路由表

普通路由器只有一张全局路由表。创建 VRF 后，接口绑定 VRF，它的直连路由、静态路由都进入**该 VRF 专属的路由表**；不同 VRF 可以有完全相同的 192.168.1.0/24，互不冲突。VRF 之间要通，必须显式引入路由（本模拟器用静态 LSP 的 vrf 参数完成映射）。

```
华为/华三：
ip vpn-instance CUST1
interface g0/0/0
 ip binding vpn-instance CUST1
 ip address 192.168.1.1 24
思科：
vrf definition CUST1
interface g0/0/0
 ip vrf forwarding CUST1
 ip address 192.168.1.1 255.255.255.0
```

## MPLS：标签栈怎么走

入口 PE 给报文压入**两层标签**：外层是传输标签（让 P 路由器逐跳转发），内层是 VPN 标签（告诉出口 PE 交给哪个 VRF）。P 路由器只看外层、换标签；出口 PE 弹出外层，按内层标签把报文投进对应 VRF——这就是"标签交换"比逐跳查 IP 快的原因。

本模拟器用**静态 LSP** 模拟这条路径：

```
PE1：
static-lsp ingress to-ce2 destination 192.168.2.0 24 \
  nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001
P：
static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200
PE2：
static-lsp egress vpn1001 incoming-label 1001 vrf CUST1
```

思科写法是在前面加 `mpls`：`mpls static-lsp ingress ...`。回程要单独再配一套 LSP——MPLS 不会自动帮你建反向路径。

## 动手

打开实验八：按任务把 PE1、P、PE2 的 VRF 与去程/回程 LSP 配全。`display mpls lsp` / `show mpls lsp` 应看到 LSP 的 VRF 与 VPN 标签；CE1 与 CE2 互相 ping 通，抓包可见外层标签在每一跳被替换。
