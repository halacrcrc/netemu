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
        Mode::If
        | Mode::Pool
        | Mode::Acl
        | Mode::Line
        | Mode::Aaa
        | Mode::Mst
        | Mode::Ospf
        | Mode::OspfArea => Mode::Config,
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

/// undo 子树：`undo shutdown` / `undo ip address` / `undo eth-trunk`
fn undo_branch() -> CNode {
    CNode::lit("undo", "Negate a command or set its defaults")
        .kid(CNode::lit("shutdown", "Undo the shutdown state").run(h_undo_shutdown))
        .kid(
            CNode::lit("ip", "IP information")
                .kid(CNode::lit("address", "Remove the IP address").run(cmd::no_ip_address)),
        )
        .kid(CNode::lit("eth-trunk", "Leave the Eth-Trunk").run(cmd::lag_unbind))
        .kid(
            CNode::lit("port", "Port commands").kid(
                CNode::lit("link-aggregation", "Link aggregation")
                    .kid(CNode::lit("group", "Leave the aggregation group").run(cmd::lag_unbind)),
            ),
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
        .kid(
            CNode::lit("ospf", "Display OSPF information")
                .kid(CNode::lit("peer", "OSPF neighbors").run(cmd::show_ospf_neighbor))
                .kid(CNode::lit("lsdb", "OSPF link state database").run(cmd::show_ospf_db)),
        )
        .kid(CNode::lit("vrrp", "Display VRRP information").run(cmd::show_vrrp))
        .kid(
            CNode::lit("ipv6", "Display IPv6 information")
                .kid(CNode::lit("interface", "IPv6 interfaces").kid(
                    CNode::lit("brief", "Brief IPv6 interface status").run(cmd::show_ipv6_brief),
                ))
                .kid(CNode::lit("routing-table", "IPv6 routing table").run(cmd::show_ipv6_route)),
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
        .kid(
            CNode::lit("eth-trunk", "Display Eth-Trunk information")
                .run(cmd::show_lag)
                .kid(
                    CNode::param(ParamKind::Number, "group", "Eth-Trunk ID, 1-128")
                        .run(cmd::show_lag),
                ),
        )
        .kid(
            CNode::lit("link-aggregation", "Display link aggregation information")
                .run(cmd::show_lag)
                .kid(CNode::lit("summary", "Summary of aggregation groups").run(cmd::show_lag))
                .kid(CNode::lit("verbose", "Detailed aggregation information").run(cmd::show_lag)),
        )
}

/// 本地用户配置（VRP / Comware 两种子树共用）。
fn local_user_node() -> CNode {
    CNode::lit("local-user", "Configure a local user").kid(
        CNode::param(ParamKind::Word, "name", "User name")
            .kid(
                CNode::lit("class", "User class (Comware)")
                    .kid(CNode::lit("manage", "Manage class").run(cmd::mgmt_local_user_enter)),
            )
            .kid(
                CNode::lit("password", "Set the user password")
                    .kid(
                        CNode::lit("cipher", "Cipher text").kid(
                            CNode::param(ParamKind::Word, "secret", "Password")
                                .run(cmd::mgmt_user_password),
                        ),
                    )
                    .kid(
                        CNode::lit("simple", "Plain text (Comware)").kid(
                            CNode::param(ParamKind::Word, "secret", "Password")
                                .run(cmd::mgmt_user_password),
                        ),
                    )
                    .kid(
                        CNode::lit("irreversible-cipher", "Irreversible cipher (VRP)").kid(
                            CNode::param(ParamKind::Word, "secret", "Password")
                                .run(cmd::mgmt_user_password),
                        ),
                    ),
            )
            .kid(
                CNode::lit("privilege", "Set the privilege level").kid(
                    CNode::lit("level", "Privilege level").kid(
                        CNode::param(ParamKind::Number, "privilege", "0-15 level")
                            .run(cmd::mgmt_user_privilege),
                    ),
                ),
            )
            .kid(
                CNode::lit("service-type", "Allowed service types").kid(
                    CNode::param(ParamKind::WordList, "services", "telnet | ssh | all")
                        .run(cmd::mgmt_user_service),
                ),
            )
            .kid(
                CNode::lit(
                    "authorization-attribute",
                    "Authorization attributes (Comware)",
                )
                .kid(
                    CNode::lit("user-role", "User role").kid(
                        CNode::param(ParamKind::Word, "role", "network-admin | network-operator")
                            .run(cmd::mgmt_user_role),
                    ),
                ),
            ),
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
            .kid(
                CNode::lit("ipv6", "IPv6 connectivity").kid(
                    CNode::param(ParamKind::Word, "target", "IPv6 address of target")
                        .run(cmd::ping_ipv6),
                ),
            )
            .run(cmd::ping)
    };
    let login_cmds = || {
        vec![
            CNode::lit("telnet", "Open a Telnet connection")
                .kid(CNode::param(
                    ParamKind::Ipv4,
                    "target",
                    "IP address of target",
                ))
                .run(cmd::telnet_login),
            CNode::lit("ssh", "Open an SSH connection")
                .kid(CNode::param(
                    ParamKind::Ipv4,
                    "target",
                    "IP address of target",
                ))
                .run(cmd::ssh_login),
        ]
    };

    Trees {
        exec: vec![
            CNode::lit("system-view", "Enter the system view").run(h_system_view),
            ping(),
            display_branch(),
            CNode::lit("save", "Save the current configuration").run(cmd::save_msg),
        ]
        .into_iter()
        .chain(login_cmds())
        .collect(),
        priv_: vec![], // VRP/Comware 无特权模式
        config: vec![
            CNode::lit("sysname", "Set the system name")
                .kid(CNode::param(ParamKind::Word, "name", "System name"))
                .run(cmd::hostname),
            CNode::lit("ospf", "OSPF process")
                .kid(CNode::param(ParamKind::Number, "pid", "Process ID").run(cmd::ospf_enter)),
            CNode::lit("aaa", "Authentication, authorization and accounting")
                .run(cmd::mgmt_enter_aaa),
            local_user_node(),
            CNode::lit("user-interface", "Configure terminal user interfaces").kid(
                CNode::lit("vty", "Virtual terminal lines").kid(
                    CNode::param(ParamKind::Number, "first", "First line number").kid(
                        CNode::param(ParamKind::Number, "last", "Last line number")
                            .run(cmd::mgmt_enter_line),
                    ),
                ),
            ),
            CNode::lit("line", "Configure terminal lines (Comware)").kid(
                CNode::lit("vty", "Virtual terminal lines").kid(
                    CNode::param(ParamKind::Number, "first", "First line number").kid(
                        CNode::param(ParamKind::Number, "last", "Last line number")
                            .run(cmd::mgmt_enter_line),
                    ),
                ),
            ),
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
            CNode::lit("ipv6", "IPv6 configuration").kid(
                CNode::lit("route-static", "IPv6 static route").kid(
                    CNode::param(ParamKind::Word, "prefix", "Prefix, e.g. 2001:db8:2::/64")
                        .kid(
                            CNode::param(ParamKind::Number, "len", "Prefix length").kid(
                                CNode::param(ParamKind::Word, "nexthop", "Next hop IPv6 address")
                                    .run(cmd::ipv6_route),
                            ),
                        )
                        .kid(
                            CNode::param(ParamKind::Word, "nexthop", "Next hop IPv6 address")
                                .run(cmd::ipv6_route),
                        ),
                ),
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
                    CNode::lit("mode", "Spanning tree mode")
                        .kid(CNode::lit("rstp", "Rapid Spanning Tree").run(cmd::stp_mode_rstp))
                        .kid(CNode::lit("stp", "802.1D Spanning Tree").run(cmd::stp_mode_stp))
                        .kid(CNode::lit("mstp", "Multiple Spanning Tree").run(cmd::stp_mode_mst)),
                )
                .kid(
                    CNode::lit("instance", "MST instance").kid(
                        CNode::param(ParamKind::Number, "id", "MST instance, 1-4094").kid(
                            CNode::lit("priority", "Set the instance bridge priority").kid(
                                CNode::param(ParamKind::Number, "priority", "0-61440, step 4096")
                                    .run(cmd::stp_mst_priority),
                            ),
                        ),
                    ),
                )
                .kid(
                    CNode::lit("region-configuration", "Enter MST region configuration")
                        .run(cmd::stp_enter_mst),
                )
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
                )
                .kid(
                    CNode::lit("eth-trunk", "Eth-Trunk interface (VRP)").kid(
                        CNode::param(ParamKind::Number, "lag", "Eth-Trunk ID, 1-128")
                            .run(cmd::interface_lag),
                    ),
                )
                .kid(
                    CNode::lit(
                        "bridge-aggregation",
                        "Bridge-Aggregation interface (Comware)",
                    )
                    .kid(
                        CNode::param(ParamKind::Number, "lag", "Aggregation ID, 1-128")
                            .run(cmd::interface_lag),
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
                )
                .kid(
                    CNode::lit("link-aggregation", "Link aggregation (Comware)").kid(
                        CNode::lit("group", "Join an aggregation group").kid(
                            CNode::param(ParamKind::Number, "group", "Aggregation group, 1-128")
                                .run(cmd::lag_bind_dynamic),
                        ),
                    ),
                ),
            CNode::lit("eth-trunk", "Join an Eth-Trunk (VRP)").kid(
                CNode::param(ParamKind::Number, "group", "Eth-Trunk ID, 1-128")
                    .run(cmd::lag_bind_dynamic),
            ),
            CNode::lit("mode", "Working mode of the aggregation interface")
                .kid(
                    CNode::lit("lacp-static", "LACP static aggregation mode (VRP)")
                        .run(cmd::lag_mode_lacp),
                )
                .kid(
                    CNode::lit("manual", "Manual load balancing mode (VRP)").kid(
                        CNode::lit("load-balance", "Manual load balancing")
                            .run(cmd::lag_mode_static),
                    ),
                ),
            CNode::lit("link-aggregation", "Link aggregation (Comware)").kid(
                CNode::lit("mode", "Aggregation working mode")
                    .kid(CNode::lit("dynamic", "LACP dynamic mode").run(cmd::lag_mode_lacp))
                    .kid(CNode::lit("static", "Static aggregation mode").run(cmd::lag_mode_static)),
            ),
            CNode::lit("ipv6", "IPv6 interface configuration").kid(
                CNode::lit("address", "Set the IPv6 address").kid(
                    CNode::param(
                        ParamKind::Word,
                        "addr",
                        "Address/prefix, e.g. 2001:db8::1/64",
                    )
                    .run(cmd::ipv6_address),
                ),
            ),
            CNode::lit("stp", "Spanning Tree Protocol subgroup").kid(
                CNode::lit("cost", "Set the port path cost").kid(
                    CNode::param(ParamKind::Number, "cost", "Path cost, 1-200000000")
                        .run(cmd::stp_cost),
                ),
            ),
            CNode::lit("vrrp", "VRRP virtual router").kid(
                CNode::lit("vrid", "VRRP group").kid(
                    CNode::param(ParamKind::Number, "vrid", "VRRP group, 1-255")
                        .kid(CNode::lit("virtual-ip", "Virtual IP address").kid(
                            CNode::param(ParamKind::Ipv4, "vip", "Virtual IP").run(cmd::vrrp_vip),
                        ))
                        .kid(
                            CNode::lit("priority", "Set the priority").kid(
                                CNode::param(ParamKind::Number, "priority", "1-254")
                                    .run(cmd::vrrp_priority),
                            ),
                        )
                        .kid(
                            CNode::lit("preempt-mode", "Enable preemption").run(cmd::vrrp_preempt),
                        ),
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
        line: vec![
            CNode::lit("authentication-mode", "Set the authentication mode").kid(
                CNode::param(ParamKind::Word, "mode", "aaa | scheme | password | none")
                    .run(cmd::mgmt_auth_mode),
            ),
            CNode::lit("protocol", "Protocols allowed on this line").kid(
                CNode::lit("inbound", "Inbound protocols").kid(
                    CNode::param(ParamKind::WordList, "protos", "ssh | telnet | all | none")
                        .run(cmd::mgmt_transport),
                ),
            ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
        aaa: vec![
            local_user_node(),
            CNode::lit("password", "Set the password of the current user")
                .kid(
                    CNode::lit("simple", "Plain text").kid(
                        CNode::param(ParamKind::Word, "secret", "Password")
                            .run(cmd::mgmt_user_password),
                    ),
                )
                .kid(
                    CNode::lit("cipher", "Cipher text").kid(
                        CNode::param(ParamKind::Word, "secret", "Password")
                            .run(cmd::mgmt_user_password),
                    ),
                ),
            CNode::lit("service-type", "Allowed service types").kid(
                CNode::param(ParamKind::WordList, "services", "telnet | ssh | all")
                    .run(cmd::mgmt_user_service),
            ),
            CNode::lit("authorization-attribute", "Authorization attributes").kid(
                CNode::lit("user-role", "User role").kid(
                    CNode::param(ParamKind::Word, "role", "network-admin | network-operator")
                        .run(cmd::mgmt_user_role),
                ),
            ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
        mst: vec![
            CNode::lit("instance", "Map VLANs to an MST instance").kid(
                CNode::param(ParamKind::Number, "id", "MST instance, 1-4094").kid(
                    CNode::lit("vlan", "VLAN list").kid(
                        CNode::param(ParamKind::VlanList, "vlans", "VLAN list, e.g. 10 20-24")
                            .run(cmd::stp_mst_instance_vlan),
                    ),
                ),
            ),
            CNode::lit("region-name", "Region name")
                .kid(CNode::param(ParamKind::Word, "name", "Region name").run(cmd::stp_mst_noop)),
            CNode::lit("revision-level", "Region revision level").kid(
                CNode::param(ParamKind::Number, "rev", "Revision, 0-65535").run(cmd::stp_mst_noop),
            ),
            CNode::lit("active", "Activate the region configuration").kid(
                CNode::lit("region-configuration", "Activate region configuration")
                    .run(cmd::stp_mst_noop),
            ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
        ospf: vec![
            CNode::lit("area", "Enter OSPF area view").kid(
                CNode::param(ParamKind::Word, "area", "Area ID, e.g. 0 or 0.0.0.0")
                    .run(cmd::ospf_area_enter),
            ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
        ospf_area: vec![
            CNode::lit("network", "Enable OSPF on a network").kid(
                CNode::param(ParamKind::Ipv4, "net", "Network address").kid(
                    CNode::param(ParamKind::Wildcard, "wildcard", "Wildcard bits")
                        .run(cmd::ospf_network),
                ),
            ),
            CNode::lit("quit", "Quit from the current view").run(h_quit),
            CNode::lit("return", "Return to the user view").run(h_return),
        ],
    }
}
