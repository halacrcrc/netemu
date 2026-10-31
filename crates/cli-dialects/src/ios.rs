//! 思科 IOS 风格命令树。

use cli_engine::{CNode, CliError, Ctx, Mode, ParamKind};
use sim_core::device::Dialect;

use crate::cmd;
use crate::Trees;

fn h_enable(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Priv;
    c.sess.sub = Default::default();
    Ok(String::new())
}

fn h_disable(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Exec;
    c.sess.cur_if = None;
    Ok(String::new())
}

fn h_conf_t(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Config;
    Ok(String::new())
}

fn h_end(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = Mode::Priv;
    c.sess.cur_if = None;
    c.sess.sub = Default::default();
    Ok(String::new())
}

fn h_exit(c: &mut Ctx) -> Result<String, CliError> {
    c.sess.mode = match c.sess.mode {
        Mode::If => Mode::Config,
        Mode::Config => Mode::Priv,
        Mode::Pool | Mode::Acl => Mode::Config,
        _ => Mode::Exec,
    };
    if c.sess.mode != Mode::If {
        c.sess.cur_if = None;
        c.sess.sub = Default::default();
    }
    Ok(String::new())
}

fn h_write(_c: &mut Ctx) -> Result<String, CliError> {
    Ok("Building configuration...\n[OK]".into())
}

fn h_no_shutdown(c: &mut Ctx) -> Result<String, CliError> {
    cmd::shutdown(c, true)
}

fn h_shutdown(c: &mut Ctx) -> Result<String, CliError> {
    cmd::shutdown(c, false)
}

fn show_branch() -> CNode {
    CNode::lit("show", "Show running system information")
        .kid(
            CNode::lit("ip", "IP information")
                .kid(
                    CNode::lit("interface", "Interface status and configuration").kid(
                        CNode::lit("brief", "Brief summary of IP status and configuration")
                            .run(cmd::show_brief),
                    ),
                )
                .kid(CNode::lit("route", "IP routing table").run(cmd::show_route))
                .kid(
                    CNode::lit("nat", "NAT translations")
                        .kid(
                            CNode::lit("translations", "Show NAT translation table")
                                .run(cmd::show_nat),
                        )
                        .run(cmd::show_nat),
                ),
        )
        .kid(CNode::lit("spanning-tree", "Spanning tree topology").run(cmd::show_stp))
        .kid(CNode::lit("access-lists", "List access lists").run(cmd::show_acl))
        .kid(
            CNode::lit("vlan", "VLAN information")
                .kid(CNode::lit("brief", "Show all VLANs and port membership").run(cmd::show_vlan)),
        )
        .kid(
            CNode::lit("mac", "MAC address table information")
                .kid(CNode::lit("address-table", "Show the MAC address table").run(cmd::show_mac)),
        )
}

pub fn build() -> Trees {
    let _ = Dialect::Ios;
    let ping = || {
        CNode::lit("ping", "Send echo messages")
            .kid(CNode::param(
                ParamKind::Ipv4,
                "target",
                "IP address of target",
            ))
            .run(cmd::ping)
    };

    Trees {
        exec: vec![
            CNode::lit("enable", "Turn on privileged commands").run(h_enable),
            ping(),
            show_branch(),
        ],
        priv_: vec![
            CNode::lit("disable", "Turn off privileged commands").run(h_disable),
            CNode::lit("configure", "Enter configuration mode")
                .kid(CNode::lit("terminal", "Configure from the terminal").run(h_conf_t)),
            CNode::lit("write", "Write running configuration to memory")
                .run(h_write)
                .kid(CNode::lit("memory", "Write configuration to memory").run(h_write)),
            ping(),
            show_branch(),
        ],
        config: vec![
            CNode::lit("hostname", "Set system's network name")
                .kid(CNode::param(
                    ParamKind::Word,
                    "name",
                    "Hostname of the router",
                ))
                .run(cmd::hostname),
            CNode::lit("interface", "Select an interface to configure")
                .run(cmd::interface)
                .kid(CNode::param(
                    ParamKind::IfName,
                    "if",
                    "e.g. GigabitEthernet0/0, Vlan10",
                ))
                .kid(
                    CNode::lit("vlan", "VLAN interface (SVI)").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                            .run(cmd::interface_svi),
                    ),
                ),
            CNode::lit("vlan", "VLAN commands")
                .run(cmd::create_vlans)
                .kid(CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")),
            CNode::lit("ip", "Global IP configuration subcommands")
                .kid(
                    CNode::lit("route", "Establish static routes")
                        .run(cmd::static_route)
                        .kid(
                            CNode::param(ParamKind::Ipv4, "dst", "Destination prefix").kid(
                                CNode::param(ParamKind::DottedMask, "mask", "Destination mask")
                                    .kid(CNode::param(
                                        ParamKind::Ipv4,
                                        "nexthop",
                                        "Next-hop IP address",
                                    )),
                            ),
                        ),
                )
                .kid(
                    CNode::lit("dhcp", "Configure DHCP server and relay").kid(
                        CNode::lit("pool", "Configure a DHCP address pool")
                            .kid(CNode::param(ParamKind::Word, "name", "Name of the pool"))
                            .run(cmd::dhcp_pool_enter),
                    ),
                )
                .kid(
                    CNode::lit("nat", "NAT configuration commands")
                        .kid(
                            CNode::lit("outside", "Mark this interface as outside for NAT")
                                .run(cmd::nat_mark_outside),
                        )
                        .kid(
                            CNode::lit("inside", "Inside address translation")
                                .run(cmd::nat_mark_inside)
                                .kid(
                                    CNode::lit("source", "Source translation")
                                        .kid(
                                            CNode::lit("list", "Specify an access list").kid(
                                                CNode::param(
                                                    ParamKind::Number,
                                                    "acl",
                                                    "ACL number",
                                                )
                                                .kid(
                                                    CNode::lit(
                                                        "interface",
                                                        "Specify an interface for global address",
                                                    )
                                                    .kid(
                                                        CNode::param(
                                                            ParamKind::IfName,
                                                            "if",
                                                            "Outside interface",
                                                        )
                                                        .run(cmd::nat_easyip_ios),
                                                    ),
                                                ),
                                            ),
                                        )
                                        .kid(
                                            CNode::lit("static", "Static address translation").kid(
                                                CNode::param(
                                                    ParamKind::Ipv4,
                                                    "inside",
                                                    "Inside local IP",
                                                )
                                                .kid(
                                                    CNode::param(
                                                        ParamKind::Ipv4,
                                                        "outside",
                                                        "Inside global IP",
                                                    )
                                                    .run(cmd::nat_static),
                                                ),
                                            ),
                                        ),
                                ),
                        ),
                ),
            CNode::lit("access-list", "Add an access list entry").kid(
                CNode::param(
                    ParamKind::Number,
                    "number",
                    "1-99 standard, 100-199 extended",
                )
                .run(cmd::ios_access_list)
                .kid(
                    CNode::param(ParamKind::Word, "action", "deny | permit").kid(
                        CNode::param(ParamKind::Word, "proto", "icmp | (omitted for IP)").kid(
                            CNode::param(ParamKind::Ipv4, "src", "Source address").kid(
                                CNode::param(
                                    ParamKind::Wildcard,
                                    "src_wildcard",
                                    "Source wildcard bits",
                                )
                                .kid(
                                    CNode::param(ParamKind::Ipv4, "dst", "Destination address")
                                        .kid(
                                            CNode::param(
                                                ParamKind::Wildcard,
                                                "dst_wildcard",
                                                "Destination wildcard bits",
                                            )
                                            .run(cmd::ios_access_list),
                                        ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
            CNode::lit("spanning-tree", "Spanning Tree Protocol subgroup")
                .run(|c| cmd::stp_enable(c, true))
                .kid(
                    CNode::lit("vlan", "VLAN spanning tree configuration").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094").kid(
                            CNode::lit("priority", "Set the bridge priority for the spanning tree")
                                .kid(
                                    CNode::param(
                                        ParamKind::Number,
                                        "priority",
                                        "0-61440, step 4096",
                                    )
                                    .run(cmd::stp_priority),
                                ),
                        ),
                    ),
                ),
            CNode::lit("no", "Negate a command or set its defaults").kid(
                CNode::lit("spanning-tree", "Disable spanning tree").kid(
                    CNode::lit("vlan", "VLAN spanning tree configuration").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                            .run(|c| cmd::stp_enable(c, false)),
                    ),
                ),
            ),
            CNode::lit("exit", "Exit from configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        if_: vec![
            CNode::lit("ip", "Interface IP configuration subcommands")
                .kid(
                    CNode::lit("address", "Set the IP address of the interface")
                        .run(cmd::ip_address)
                        .kid(
                            CNode::param(ParamKind::Ipv4, "ip", "IP address").kid(CNode::param(
                                ParamKind::DottedMask,
                                "mask",
                                "Subnet mask",
                            )),
                        ),
                )
                .kid(
                    CNode::lit("nat", "NAT configuration")
                        .kid(
                            CNode::lit("inside", "Mark this interface as inside for NAT")
                                .run(cmd::nat_mark_inside),
                        )
                        .kid(
                            CNode::lit("outside", "Mark this interface as outside for NAT")
                                .run(cmd::nat_mark_outside),
                        ),
                )
                .kid(
                    CNode::lit("access-group", "Apply an access list to the interface").kid(
                        CNode::param(ParamKind::Number, "acl", "ACL number").kid(
                            CNode::param(ParamKind::Word, "dir", "in | out").run(cmd::acl_apply),
                        ),
                    ),
                ),
            // 交换机二层口命令
            CNode::lit("switchport", "Set switching mode characteristics")
                .kid(
                    CNode::lit("mode", "Set trunking mode of the interface")
                        .kid(
                            CNode::lit("access", "Set access mode (untagged, one VLAN)")
                                .run(cmd::sw_mode_access),
                        )
                        .kid(
                            CNode::lit("trunk", "Set trunk mode (tagged, multiple VLANs)")
                                .run(cmd::sw_mode_trunk),
                        ),
                )
                .kid(
                    CNode::lit("access", "Assign access VLAN").kid(
                        CNode::lit("vlan", "Set the access VLAN").kid(
                            CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094")
                                .run(cmd::access_vlan),
                        ),
                    ),
                )
                .kid(
                    CNode::lit("trunk", "Trunk characteristics").kid(
                        CNode::lit("allowed", "Set allowed VLANs when in trunk mode").kid(
                            CNode::lit("vlan", "Set VLANs allowed on trunk").kid(
                                CNode::param(
                                    ParamKind::VlanList,
                                    "vlans",
                                    "VLAN list, e.g. 10,20-24",
                                )
                                .run(cmd::trunk_allow),
                            ),
                        ),
                    ),
                ),
            CNode::lit("spanning-tree", "Spanning Tree Protocol subgroup").kid(
                CNode::lit("vlan", "VLAN spanning tree configuration").kid(
                    CNode::param(ParamKind::Number, "vlan", "VLAN ID, 1-4094").kid(
                        CNode::lit("cost", "Set the port path cost").kid(
                            CNode::param(ParamKind::Number, "cost", "Path cost, 1-200000000")
                                .run(cmd::stp_cost),
                        ),
                    ),
                ),
            ),
            CNode::lit("no", "Negate a command or set its defaults")
                .kid(CNode::lit("shutdown", "Undo shutdown of the interface").run(h_no_shutdown))
                .kid(
                    CNode::lit("ip", "IP information").kid(
                        CNode::lit("address", "Remove the IP address").run(cmd::no_ip_address),
                    ),
                ),
            CNode::lit("shutdown", "Shut down the interface").run(h_shutdown),
            CNode::lit("exit", "Exit from interface configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        pool: vec![
            CNode::lit("network", "Configure the subnet of the pool")
                .run(cmd::pool_network)
                .kid(
                    CNode::param(ParamKind::Ipv4, "net", "Network address").kid(
                        CNode::param(ParamKind::DottedMask, "mask", "Subnet mask")
                            .run(cmd::pool_network),
                    ),
                ),
            CNode::lit("default-router", "Configure the default gateway")
                .kid(CNode::param(ParamKind::Ipv4, "gw", "Gateway address").run(cmd::pool_gateway)),
            CNode::lit("dns-server", "Configure the DNS server")
                .kid(CNode::param(ParamKind::Ipv4, "dns", "DNS server address").run(cmd::pool_dns)),
            CNode::lit("exit", "Exit from DHCP pool configuration mode").run(h_exit),
        ],
        acl: vec![CNode::lit("exit", "Exit from ACL configuration mode").run(h_exit)],
    }
}
