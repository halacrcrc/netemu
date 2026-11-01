---
title: 同 VLAN 再隔离：MUX VLAN · 私有 VLAN · 端口隔离
order: 43
level: MAP
topic: isolation
lab: port-isolation
---

一个 VLAN 里既有需要互通的设备，也有必须互相隔离的设备。再划 VLAN 会让网段和网关数量爆炸，于是有了**在同一个广播域内再做二层隔离**的技术。思科、华为、华三的叫法不同，目标一致。

## 三家叫什么

```
思科     Private VLAN（PVLAN）
         primary / isolated / community VLAN
         promiscuous / isolated / community 端口
华为     MUX VLAN
         principal / separate / group VLAN
         principal / separate / group 端口
华三     端口隔离（port-isolate）
         也可用 PVLAN/MUX VLAN 方案
```

典型行为：

- **principal / promiscuous**：可以和所有端口互通，通常接网关或服务器；
- **separate / isolated**：彼此不能互通，只能访问 principal；
- **group / community**：同组内互通，不能访问其他组，只能访问 principal。

## 本模拟器的模型

模拟器把两者统一成端口角色与隔离组，数据面在二层转发时检查：

```
mux-vlan principal          # 与所有端口互通
mux-vlan separate           # separate 之间、与 group 之间隔离
mux-vlan group 1            # 同组互通，跨组隔离
port-isolate enable group 1 # 同隔离组不能互通
switchport protected        # 思科 protected，等同于两两隔离
```

查看：`display mux-vlan` / `display port-isolate`；思科对应 `show mux-vlan` / `show port-isolate`。

## 常见设计

- 酒店/宿舍：每个房间一个 separate 端口，只能访问 principal 网关；
- 园区访客：isolated 端口只能到网关，不能互访；
- 服务器区：community 组内容器互通，组间隔离；
- 端口隔离：接入层批量隔离，配置比 PVLAN 简单。

注意：隔离是**二层**行为；如果允许通过网关做三层转发，隔离端口之间仍可能经网关互通，需要配合 ACL。

## 动手

打开实验二十：PC1（separate）、PC2（group 1）、PC3（principal）。配置后 PC1 与 PC2 不通，但两者都能访问 PC3。再把 PC1/PC2 改成同一 `port-isolate` 组，观察同样的隔离效果；最后把 PC3 改成 separate，看看它是否也被隔离。
