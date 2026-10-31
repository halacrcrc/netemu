//! 华为 VRP / 新华三 Comware 命令树（两方言结构高度相似，共用构建）。
//!
//! 差异（接口名规则、show 输出、报错前缀、VLAN 命令词）分别在 ifname/show/lib 中按方言处理。

use cli_engine::{CNode, CliError, Ctx, Mode, ParamKind};

use crate::cmd;
use crate::Trees;

fn h_system_view(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Config;
    Ok(String::new())
}

fn h_quit(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = match c.sess.mode {
        Mode::If | Mode::Pool | Mode::Acl => Mode::Config,
        _ => Mode::Exec,
    };
    if c.sess.mode != Mode::If {
        c.sess.cur_if = None;
        c.sess.sub = Default::default();
    }
    Ok(String::new())
}

fn h_return(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Exec;
    c.sess.cur_if = None;
    c.sess.sub = Default::default();
    Ok(String::new())
}

fn h_undo_shutdown(c: &mut Ctx) -> Result<String, CliError> {
    cmd::shutdown(c, true)
}

fn h_shutdown(c: &mut Ctx) -> Result<String, CliError> {
    cmd::shutdown(c, false)
}

/// undo 子树：`undo shutdown` / `undo ip address`
fn undo_branch() -> CNode {
    CNode::lit("undo", "Negate a command or set its defaults")
        .kid(CNode::lit("shutdown", "Undo the shutdown state").run(h_undo_shutdown))
        .kid(
            CNode::lit("ip", "IP information")
                .kid(CNode::lit("address", "Remove the IP address").run(cmd::no_ip_address)),
        )
}

fn display_branch() -> CNode {
    CNode::lit("display", "Display information")
        .kid(
            CNode::lit("ip", "IP information")
                .kid(CNode::lit("interface", "Interface status").kid(
                    CNode::lit("brief", "Brief information of interfaces").run(cmd::show_brief),
                ))
                .kid(
                    CNode::lit("routing-table", "Display the IP routing table")
                        .run(cmd::show_route),
                ),
        )
        .kid(CNode::lit("vlan", "Display VLAN information").run(cmd::show_vlan))
        .kid(CNode::lit("mac-address", "Display the MAC address table").run(cmd::show_mac))
        .kid(
            CNode::lit("acl", "Display ACL information")
                .run(cmd::show_acl)
                .kid(CNode::lit("all", "Display all ACLs").run(cmd::show_acl)),
        )
        .kid(
            CNode::lit("nat", "Display NAT information")
                .run(cmd::show_nat)
                .kid(CNode::lit("session", "Display NAT sessions").run(cmd::show_nat)),
        )
        .kid(
            CNode::lit("stp", "Display spanning tree information")
                .run(cmd::show_stp)
                .kid(CNode::lit("brief", "Brief information of spanning tree").run(cmd::show_stp)),
        )
}

pub fn build() -> Trees {
    let ping = || {
        CNode::lit("ping", "Check the network connectivity")
            .kid(CNode::param(
                ParamKind::Ipv4,
                "target",
                "IP address of target",
            ))
            .run(cmd::ping)
    };

    Trees {
        exec: vec![
            CNode::lit("system-view", "Enter the system view").run(h_system_view),
            ping(),
            display_branch(),
            CNode::lit("save", "Save the current configuration").run(cmd::save_msg),
        ],
        priv_: vec![], // VRP/Comware 无特权模式
        config: vec![
            CNode::lit("sysname", "Set the system name")
                .kid(CNode::param(ParamKind::Word, "name", "System name"))
                .run(cmd::hostname),
            CNode::lit("vlan", "VLAN commands")
                .run(cmd::create_vlans)
                .kid(CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094"))
                .kid(
                    CNode::lit("batch", "Create VLANs in batch")
                        .run(cmd::create_vlans_list)
                        .kid(CNode::param(
                            ParamKind::VlanList,
                            "vlans",
                            "VLAN list, e.g. 10 20-24",
                        )),
                ),
            CNode::lit("ip", "IP configuration subcommands")
                .kid(
                    CNode::lit("route-static", "Establish static routes")
                        .run(cmd::static_route)
                        .kid(
                            CNode::param(ParamKind::Ipv4, "dst", "Destination prefix").kid(
                                CNode::param(ParamKind::PrefixLen, "mask", "Prefix length").kid(
                                    CNode::param(ParamKind::Ipv4, "nexthop", "Next-hop IP address"),
                                ),
                            ),
                        ),
                )
                .kid(
                    CNode::lit("pool", "Create a DHCP address pool")
                        .kid(CNode::param(ParamKind::Word, "name", "Name of the pool"))
                        .run(cmd::dhcp_pool_enter),
                ),
            CNode::lit("acl", "Create an ACL and enter ACL view")
                .kid(
                    CNode::param(
                        ParamKind::Number,
                        "number",
                        "2000-2999 basic, 3000-3999 advanced",
                    )
                    .run(cmd::acl_enter),
                )
                .kid(
                    CNode::param(ParamKind::Word, "kind", "basic | advanced").kid(
                        CNode::param(
                            ParamKind::Number,
                            "number",
                            "2000-2999 basic, 3000-3999 advanced",
                        )
                        .run(cmd::acl_enter),
                    ),
                ),
            CNode::lit("nat", "NAT configuration").kid(
                CNode::lit("static", "Static NAT").run(cmd::nat_static).kid(
                    CNode::lit("global", "Specify the global (public) IP").kid(
                        CNode::param(ParamKind::Ipv4, "outside", "Inside global IP").kid(
                            CNode::lit("inside", "Specify the inside (private) IP").kid(
                                CNode::param(ParamKind::Ipv4, "inside", "Inside local IP")
                                    .run(cmd::nat_static),
                            ),
                        ),
                    ),
                ),
            ),
            CNode::lit("stp", "Spanning Tree Protocol subgroup")
                .run(|c| cmd::stp_enable(c, true))
                .kid(
                    CNode::lit("priority", "Set the bridge priority").kid(
                        CNode::param(ParamKind::Number, "priority", "0-61440, step 4096")
                            .run(cmd::stp_priority),
                    ),
                )
                .kid(
                    CNode::lit("disable", "Disable the spanning tree protocol")
                        .run(|c| cmd::stp_enable(c, false)),
                ),
            CNode::lit("interface", "Enter the interface view")
                .run(cmd::interface)
                .kid(CNode::param(
                    ParamKind::IfName,
                    "if",
                    "e.g. GigabitEthernet0/0/0, Vlanif10",
                ))
                .kid(
                    CNode::lit("vlanif", "VLAN interface (SVI)").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                            .run(cmd::interface_svi),
                    ),
                )
                .kid(
                    CNode::lit("vlan-interface", "VLAN interface (SVI, Comware)").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                            .run(cmd::interface_svi),
                    ),
                ),
            undo_branch(),
            display_branch(),
            ping(),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
            CNode::lit("save", "Save the current configuration").run(cmd::save_msg),
        ],
        if_: vec![
            CNode::lit("ip", "IP configuration subcommands").kid(
                CNode::lit("address", "Set the IP address of the interface")
                    .run(cmd::ip_address)
                    .kid(
                        CNode::param(ParamKind::Ipv4, "ip", "IP address").kid(CNode::param(
                            ParamKind::PrefixLen,
                            "mask",
                            "Prefix length",
                        )),
                    ),
            ),
            CNode::lit("port", "Port commands")
                .kid(
                    CNode::lit("link-type", "Set the port link type")
                        .kid(
                            CNode::lit("access", "Access port (untagged, one VLAN)")
                                .run(cmd::lt_access),
                        )
                        .kid(
                            CNode::lit("trunk", "Trunk port (tagged, multiple VLANs)")
                                .run(cmd::lt_trunk),
                        ),
                )
                .kid(
                    CNode::lit("default", "Set the default (pvid) VLAN of the access port").kid(
                        CNode::lit("vlan", "Set the default VLAN").kid(
                            CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                                .run(cmd::access_vlan),
                        ),
                    ),
                )
                .kid(
                    CNode::lit("access", "Assign the access VLAN (Comware)").kid(
                        CNode::lit("vlan", "Set the access VLAN").kid(
                            CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                                .run(cmd::access_vlan),
                        ),
                    ),
                )
                .kid(
                    CNode::lit("trunk", "Trunk port characteristics")
                        .kid(
                            CNode::lit("allow-pass", "Allow specified VLANs on the trunk (VRP)")
                                .kid(
                                    CNode::lit("vlan", "VLAN list to allow").kid(
                                        CNode::param(
                                            ParamKind::VlanList,
                                            "vlans",
                                            "VLAN list, e.g. 10 20",
                                        )
                                        .run(cmd::trunk_allow),
                                    ),
                                ),
                        )
                        .kid(
                            CNode::lit("permit", "Permit specified VLANs on the trunk (Comware)")
                                .kid(
                                    CNode::lit("vlan", "VLAN list to permit").kid(
                                        CNode::param(
                                            ParamKind::VlanList,
                                            "vlans",
                                            "VLAN list, e.g. 10 20",
                                        )
                                        .run(cmd::trunk_allow),
                                    ),
                                ),
                        ),
                ),
            CNode::lit("stp", "Spanning Tree Protocol subgroup").kid(
                CNode::lit("cost", "Set the port path cost").kid(
                    CNode::param(ParamKind::Number, "cost", "Path cost, 1-200000000")
                        .run(cmd::stp_cost),
                ),
            ),
            CNode::lit("dhcp", "DHCP configuration").kid(
                CNode::lit("select", "Enable the DHCP server on this interface")
                    .kid(CNode::lit("global", "Use global address pools").run(cmd::dhcp_select)),
            ),
            CNode::lit("nat", "NAT configuration").kid(
                CNode::lit("outbound", "Enable Easy-IP NAT for outbound traffic")
                    .run(cmd::nat_outbound),
            ),
            CNode::lit("traffic-filter", "Apply an ACL to this interface").kid(
                CNode::param(ParamKind::Word, "dir", "inbound | outbound").kid(
                    CNode::lit("acl", "Specify an ACL").kid(
                        CNode::param(ParamKind::Number, "acl", "ACL number").run(cmd::acl_apply),
                    ),
                ),
            ),
            undo_branch(),
            CNode::lit("shutdown", "Shut down the interface").run(h_shutdown),
            display_branch(),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
        pool: vec![
            CNode::lit("network", "Configure the subnet of the pool")
                .run(cmd::pool_network)
                .kid(
                    CNode::param(ParamKind::Ipv4, "net", "Network address").kid(
                        CNode::lit("mask", "Specify the subnet mask").kid(
                            CNode::param(ParamKind::DottedMask, "mask", "Subnet mask")
                                .run(cmd::pool_network),
                        ),
                    ),
                ),
            CNode::lit("gateway-list", "Configure the gateway address")
                .kid(CNode::param(ParamKind::Ipv4, "gw", "Gateway address").run(cmd::pool_gateway)),
            CNode::lit("dns-list", "Configure the DNS server")
                .kid(CNode::param(ParamKind::Ipv4, "dns", "DNS server address").run(cmd::pool_dns)),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
        ],
        acl: vec![
            CNode::lit("rule", "Add a rule to the ACL")
                .run(cmd::acl_rule)
                .kid(
                    CNode::param(ParamKind::Word, "action", "deny | permit").kid(
                        CNode::param(ParamKind::Word, "proto", "icmp | (omitted for IP)").kid(
                            CNode::lit("source", "Specify the source address").kid(
                                CNode::param(ParamKind::Ipv4, "src", "Source address").kid(
                                    CNode::param(
                                        ParamKind::Wildcard,
                                        "src_wildcard",
                                        "Wildcard bits",
                                    )
                                    .kid(
                                        CNode::lit(
                                            "destination",
                                            "Specify the destination address",
                                        )
                                        .kid(
                                            CNode::param(
                                                ParamKind::Ipv4,
                                                "dst",
                                                "Destination address",
                                            )
                                            .kid(
                                                CNode::param(
                                                    ParamKind::Wildcard,
                                                    "dst_wildcard",
                                                    "Wildcard bits",
                                                )
                                                .run(cmd::acl_rule),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
        ],
    }
}
