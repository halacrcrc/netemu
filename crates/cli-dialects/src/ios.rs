//! 思科 IOS 风格命令树。

use cli_engine::{CNode, CliError, CmdFn, Ctx, Mode, ParamKind};
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
        Mode::Pool
        | Mode::Acl
        | Mode::Line
        | Mode::Aaa
        | Mode::Mst
        | Mode::Ospf
        | Mode::OspfArea
        | Mode::Bgp
        | Mode::Isis
        | Mode::RoutePolicy
        | Mode::Zone
        | Mode::SecurityPolicy
        | Mode::SecurityRule
        | Mode::Ipsec => Mode::Config,
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

/// `A.B.C.D wildcard [dst dst_wildcard]`（源/目的都按位置解析）。
fn acl_src_branch(handler: CmdFn) -> CNode {
    CNode::param(ParamKind::Ipv4, "src", "Source address").kid(
        CNode::param(ParamKind::Wildcard, "src_wildcard", "Source wildcard bits")
            .run(handler)
            .kid(
                CNode::param(ParamKind::Ipv4, "dst", "Destination address").kid(
                    CNode::param(
                        ParamKind::Wildcard,
                        "dst_wildcard",
                        "Destination wildcard bits",
                    )
                    .run(handler),
                ),
            ),
    )
}

/// `permit|deny {A.B.C.D w [dst w] | icmp A.B.C.D w [dst w] | ip A.B.C.D w [dst w]}`。
fn acl_action_branch() -> CNode {
    CNode::param(ParamKind::Word, "action", "deny | permit")
        .kid(acl_src_branch(cmd::ios_access_list))
        .kid(CNode::lit("icmp", "ICMP protocol").kid(acl_src_branch(cmd::ios_access_list_icmp)))
        .kid(CNode::lit("ip", "Any IP protocol").kid(acl_src_branch(cmd::ios_access_list)))
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
                .kid(
                    CNode::lit("route", "IP routing table")
                        .run(cmd::show_route)
                        .kid(
                            CNode::lit("vrf", "Show a VRF routing table").kid(
                                CNode::param(ParamKind::Word, "name", "VRF name")
                                    .run(cmd::show_route_vrf),
                            ),
                        ),
                )
                .kid(
                    CNode::lit("ospf", "OSPF information")
                        .kid(CNode::lit("neighbor", "OSPF neighbors").run(cmd::show_ospf_neighbor))
                        .kid(
                            CNode::lit("database", "OSPF link state database")
                                .run(cmd::show_ospf_db),
                        ),
                )
                .kid(
                    CNode::lit("nat", "NAT translations")
                        .kid(
                            CNode::lit("translations", "Show NAT translation table")
                                .run(cmd::show_nat),
                        )
                        .run(cmd::show_nat),
                )
                .kid(
                    CNode::lit("dhcp", "DHCP information").kid(
                        CNode::lit("snooping", "DHCP snooping").kid(
                            CNode::lit("binding", "Snooping binding table")
                                .run(cmd::show_dhcp_snooping),
                        ),
                    ),
                )
                .kid(
                    CNode::lit("bgp", "BGP information")
                        .run(cmd::show_bgp_table)
                        .kid(
                            CNode::lit("summary", "BGP neighbor summary")
                                .run(cmd::show_bgp_summary),
                        ),
                ),
        )
        .kid(
            CNode::lit("ipv6", "IPv6 information")
                .kid(CNode::lit("interface", "IPv6 interfaces").kid(
                    CNode::lit("brief", "Brief IPv6 interface status").run(cmd::show_ipv6_brief),
                ))
                .kid(CNode::lit("route", "IPv6 routing table").run(cmd::show_ipv6_route)),
        )
        .kid(CNode::lit("route-map", "Show route maps").run(cmd::show_route_policy))
        .kid(
            CNode::lit("zone", "Security zones")
                .kid(CNode::lit("security", "Show security zones").run(cmd::show_security_zone)),
        )
        .kid(
            CNode::lit("zone-pair", "Zone pairs").kid(
                CNode::lit("security", "Show zone-pair policies").run(cmd::show_security_policy),
            ),
        )
        .kid(
            CNode::lit("ipsec", "IPsec information")
                .kid(CNode::lit("tunnel", "Static tunnels").run(cmd::show_ipsec)),
        )
        .kid(
            CNode::lit("sslvpn", "SSL VPN information")
                .kid(CNode::lit("session", "SSL VPN sessions").run(cmd::show_sslvpn)),
        )
        .kid(CNode::lit("wlan", "WLAN information").run(cmd::show_wlan))
        .kid(CNode::lit("spanning-tree", "Spanning tree topology").run(cmd::show_stp))
        .kid(CNode::lit("vrrp", "VRRP status").run(cmd::show_vrrp))
        .kid(CNode::lit("port-security", "Port security").run(cmd::show_port_security))
        .kid(CNode::lit("storm-control", "Storm control").run(cmd::show_qos))
        .kid(
            CNode::lit("isis", "IS-IS information")
                .kid(CNode::lit("neighbors", "IS-IS neighbors").run(cmd::show_isis_neighbors))
                .kid(CNode::lit("database", "IS-IS LSDB").run(cmd::show_isis_db)),
        )
        .kid(
            CNode::lit("mpls", "MPLS information")
                .kid(CNode::lit("lsp", "Static LSPs").run(cmd::show_mpls)),
        )
        .kid(CNode::lit("access-lists", "List access lists").run(cmd::show_acl))
        .kid(
            CNode::lit("vlan", "VLAN information")
                .kid(CNode::lit("brief", "Show all VLANs and port membership").run(cmd::show_vlan)),
        )
        .kid(
            CNode::lit("mac", "MAC address table information")
                .kid(CNode::lit("address-table", "Show the MAC address table").run(cmd::show_mac)),
        )
        .kid(
            CNode::lit("etherchannel", "EtherChannel information").kid(
                CNode::lit("summary", "One-line summary per channel-group").run(cmd::show_lag),
            ),
        )
}

fn mpls_lsp_node() -> CNode {
    CNode::lit("static-lsp", "Static label switched path")
        .kid(
            CNode::lit("ingress", "Ingress LSR").kid(
                CNode::param(ParamKind::Word, "name", "LSP name").kid(
                    CNode::lit("destination", "FEC destination").kid(
                        CNode::param(ParamKind::Ipv4, "net", "Destination network").kid(
                            CNode::param(ParamKind::PrefixLen, "len", "Prefix length").kid(
                                CNode::lit("nexthop", "Next hop").kid(
                                    CNode::param(ParamKind::Ipv4, "nexthop", "Next hop IP").kid(
                                        CNode::lit("out-label", "Out label").kid(
                                            CNode::param(ParamKind::Number, "out", "Out label")
                                                .run(cmd::mpls_lsp_ingress)
                                                .kid(
                                                    CNode::lit("vrf", "VPN instance").kid(
                                                        CNode::param(
                                                            ParamKind::Word,
                                                            "vrf",
                                                            "VRF name",
                                                        )
                                                        .run(cmd::mpls_lsp_ingress)
                                                        .kid(
                                                            CNode::lit("vpn-label", "VPN label")
                                                                .kid(
                                                                    CNode::param(
                                                                        ParamKind::Number,
                                                                        "vpn",
                                                                        "VPN label",
                                                                    )
                                                                    .run(cmd::mpls_lsp_ingress),
                                                                ),
                                                        ),
                                                    ),
                                                )
                                                .kid(
                                                    CNode::lit("vpn-label", "VPN label").kid(
                                                        CNode::param(
                                                            ParamKind::Number,
                                                            "vpn",
                                                            "VPN label",
                                                        )
                                                        .run(cmd::mpls_lsp_ingress),
                                                    ),
                                                ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
        .kid(
            CNode::lit("transit", "Transit LSR").kid(
                CNode::param(ParamKind::Word, "name", "LSP name").kid(
                    CNode::lit("incoming-label", "Incoming label").kid(
                        CNode::param(ParamKind::Number, "in", "In label").kid(
                            CNode::lit("nexthop", "Next hop").kid(
                                CNode::param(ParamKind::Ipv4, "nexthop", "Next hop IP").kid(
                                    CNode::lit("out-label", "Out label").kid(
                                        CNode::param(ParamKind::Number, "out", "Out label")
                                            .run(cmd::mpls_lsp_transit),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        )
        .kid(
            CNode::lit("egress", "Egress LSR").kid(
                CNode::param(ParamKind::Word, "name", "LSP name").kid(
                    CNode::lit("incoming-label", "Incoming label").kid(
                        CNode::param(ParamKind::Number, "in", "In label")
                            .run(cmd::mpls_lsp_egress)
                            .kid(
                                CNode::lit("vrf", "VPN instance").kid(
                                    CNode::param(ParamKind::Word, "vrf", "VRF name")
                                        .run(cmd::mpls_lsp_egress),
                                ),
                            ),
                    ),
                ),
            ),
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
            .kid(
                CNode::lit("ipv6", "IPv6 echo messages").kid(
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
            CNode::lit("enable", "Turn on privileged commands").run(h_enable),
            ping(),
            show_branch(),
        ]
        .into_iter()
        .chain(login_cmds())
        .collect(),
        priv_: vec![
            CNode::lit("disable", "Turn off privileged commands").run(h_disable),
            CNode::lit("configure", "Enter configuration mode")
                .kid(CNode::lit("terminal", "Configure from the terminal").run(h_conf_t)),
            CNode::lit("write", "Write running configuration to memory")
                .run(h_write)
                .kid(CNode::lit("memory", "Write configuration to memory").run(h_write)),
            ping(),
            show_branch(),
        ]
        .into_iter()
        .chain(login_cmds())
        .collect(),
        config: vec![
            CNode::lit("hostname", "Set system's network name")
                .kid(CNode::param(
                    ParamKind::Word,
                    "name",
                    "Hostname of the router",
                ))
                .run(cmd::hostname),
            CNode::lit(
                "route-map",
                "Create route-map or enter route-map command mode",
            )
            .kid(
                CNode::param(ParamKind::Word, "name", "Route map tag").kid(
                    CNode::param(ParamKind::Word, "action", "permit | deny").kid(
                        CNode::param(ParamKind::Number, "seq", "Sequence number")
                            .run(cmd::route_policy_enter),
                    ),
                ),
            ),
            CNode::lit("zone", "Security zone").kid(
                CNode::lit("security", "Security zone name").kid(
                    CNode::param(ParamKind::Word, "zone", "Zone name")
                        .run(cmd::security_zone_enter),
                ),
            ),
            CNode::lit("zone-pair", "Zone pair").kid(
                CNode::lit("security", "Zone pair name").kid(
                    CNode::param(ParamKind::Word, "name", "Zone pair name")
                        .run(cmd::security_rule_enter),
                ),
            ),
            CNode::lit("ipsec", "IPsec configuration").kid(
                CNode::lit("tunnel", "Static site-to-site tunnel").kid(
                    CNode::param(ParamKind::Word, "name", "Tunnel name").kid(
                        CNode::lit("local", "Local endpoint").kid(
                            CNode::param(ParamKind::Ipv4, "local", "Local IP").kid(
                                CNode::lit("remote", "Remote endpoint").kid(
                                    CNode::param(ParamKind::Ipv4, "remote", "Remote IP")
                                        .run(cmd::ipsec_enter),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
            CNode::lit("sslvpn", "SSL VPN gateway").kid(
                CNode::lit("virtual-ip", "Virtual IP pool").kid(
                    CNode::param(ParamKind::Ipv4, "net", "Pool network").kid(
                        CNode::param(ParamKind::PrefixLen, "len", "Prefix length")
                            .run(cmd::sslvpn_pool),
                    ),
                ),
            ),
            CNode::lit("wlan", "WLAN configuration")
                .kid(
                    CNode::lit("ssid", "Set the SSID").kid(
                        CNode::param(ParamKind::Word, "ssid", "SSID name").run(cmd::wlan_ssid),
                    ),
                )
                .kid(
                    CNode::lit("security", "WLAN security")
                        .kid(CNode::lit("open", "Open network").run(cmd::wlan_security_open))
                        .kid(
                            CNode::lit("wpa2", "WPA2").kid(
                                CNode::lit("psk", "Pre-shared key").kid(
                                    CNode::param(ParamKind::Word, "psk", "Passphrase")
                                        .run(cmd::wlan_security_wpa2),
                                ),
                            ),
                        )
                        .kid(
                            CNode::lit("wpa3", "WPA3-SAE").kid(
                                CNode::lit("sae", "SAE passphrase").kid(
                                    CNode::param(ParamKind::Word, "psk", "Passphrase")
                                        .run(cmd::wlan_security_wpa3),
                                ),
                            ),
                        ),
                )
                .kid(
                    CNode::lit("vlan", "Service VLAN").kid(
                        CNode::param(ParamKind::Number, "vlan", "VLAN ID").run(cmd::wlan_vlan),
                    ),
                )
                .kid(CNode::lit("priority", "AP selection priority").kid(
                    CNode::param(ParamKind::Number, "priority", "0-255").run(cmd::wlan_priority),
                )),
            CNode::lit("capwap", "CAPWAP configuration").kid(
                CNode::lit("ac", "Controller address")
                    .kid(CNode::param(ParamKind::Ipv4, "ip", "AC IP").run(cmd::capwap_ac)),
            ),
            CNode::lit("router", "Enable a routing process")
                .kid(
                    CNode::lit("ospf", "Open Shortest Path First").kid(
                        CNode::param(ParamKind::Number, "pid", "Process ID").run(cmd::ospf_enter),
                    ),
                )
                .kid(CNode::lit("bgp", "Border Gateway Protocol").kid(
                    CNode::param(ParamKind::Number, "asn", "Local AS number").run(cmd::bgp_enter),
                ))
                .kid(CNode::lit("isis", "ISO IS-IS").run(cmd::isis_enter)),
            CNode::lit("vrf", "VRF configuration").kid(
                CNode::lit("definition", "Define a VRF")
                    .kid(CNode::param(ParamKind::Word, "name", "VRF name").run(cmd::vrf_create)),
            ),
            CNode::lit("mpls", "MPLS configuration").kid(mpls_lsp_node()),
            CNode::lit("ipv6", "IPv6 configuration").kid(
                CNode::lit("route", "IPv6 static route").kid(
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
            CNode::lit("username", "Establish a local user").kid(
                CNode::param(ParamKind::Word, "name", "User name")
                    .run(cmd::mgmt_username)
                    .kid(
                        CNode::lit("privilege", "Set the privilege level").kid(
                            CNode::param(ParamKind::Number, "privilege", "0-15").kid(
                                CNode::lit("secret", "Specify the secret").kid(
                                    CNode::param(ParamKind::Word, "secret", "Secret")
                                        .run(cmd::mgmt_username),
                                ),
                            ),
                        ),
                    )
                    .kid(CNode::lit("secret", "Specify the secret").kid(
                        CNode::param(ParamKind::Word, "secret", "Secret").run(cmd::mgmt_username),
                    )),
            ),
            CNode::lit("enable", "Configure privileged mode").kid(
                CNode::lit("secret", "Set the enable secret").kid(
                    CNode::param(ParamKind::Word, "secret", "Secret").run(cmd::mgmt_enable_secret),
                ),
            ),
            CNode::lit("line", "Configure lines").kid(
                CNode::lit("vty", "Virtual terminal lines").kid(
                    CNode::param(ParamKind::Number, "first", "First line number").kid(
                        CNode::param(ParamKind::Number, "last", "Last line number")
                            .run(cmd::mgmt_enter_line),
                    ),
                ),
            ),
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
                        )
                        .kid(
                            CNode::lit("vrf", "VRF static route").kid(
                                CNode::param(ParamKind::Word, "name", "VRF name").kid(
                                    CNode::param(ParamKind::Ipv4, "dst", "Destination").kid(
                                        CNode::param(ParamKind::DottedMask, "mask", "Mask").kid(
                                            CNode::param(ParamKind::Ipv4, "nexthop", "Next hop")
                                                .run(cmd::vrf_static),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                )
                .kid(
                    CNode::lit("dhcp", "Configure DHCP server and relay")
                        .kid(CNode::lit("snooping", "DHCP snooping").run(cmd::dhcp_snooping_enable))
                        .kid(
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
                .kid(acl_action_branch()),
            ),
            CNode::lit("spanning-tree", "Spanning Tree Protocol subgroup")
                .run(|c| cmd::stp_enable(c, true))
                .kid(
                    CNode::lit("mode", "Spanning tree mode")
                        .kid(CNode::lit("rapid-pvst", "Rapid PVST+ (RSTP)").run(cmd::stp_mode_rstp))
                        .kid(CNode::lit("pvst", "PVST+ (802.1D timers)").run(cmd::stp_mode_stp))
                        .kid(
                            CNode::lit("mst", "Multiple Spanning Tree (MSTP)")
                                .run(cmd::stp_mode_mst),
                        ),
                )
                .kid(
                    CNode::lit("mst", "MSTP configuration")
                        .kid(
                            CNode::param(ParamKind::Number, "id", "MST instance, 1-4094").kid(
                                CNode::lit("priority", "Set the instance bridge priority").kid(
                                    CNode::param(
                                        ParamKind::Number,
                                        "priority",
                                        "0-61440, step 4096",
                                    )
                                    .run(cmd::stp_mst_priority),
                                ),
                            ),
                        )
                        .kid(
                            CNode::lit("configuration", "Enter MST configuration mode")
                                .run(cmd::stp_enter_mst),
                        ),
                )
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
                )
                .kid(
                    CNode::lit("dhcp", "DHCP snooping on this interface").kid(
                        CNode::lit("snooping", "DHCP snooping").kid(
                            CNode::lit("trust", "Trust this port for DHCP servers")
                                .run(cmd::dhcp_snooping_trust),
                        ),
                    ),
                )
                .kid(CNode::lit("router", "Routing protocol").kid(
                    CNode::lit("isis", "Enable IS-IS on this interface").run(cmd::isis_enable_if),
                ))
                .kid(
                    CNode::lit("vrf", "VRF forwarding").kid(
                        CNode::lit("forwarding", "Assign this interface to a VRF").kid(
                            CNode::param(ParamKind::Word, "name", "VRF name").run(cmd::vrf_bind),
                        ),
                    ),
                ),
            CNode::lit("zone-member", "Assign this interface to a security zone").kid(
                CNode::lit("security", "Security zone").kid(
                    CNode::param(ParamKind::Word, "zone", "Zone name")
                        .run(cmd::security_zone_member),
                ),
            ),
            CNode::lit("nameif", "Assign an ASA interface name").kid(
                CNode::param(ParamKind::Word, "name", "e.g. inside, outside").run(cmd::nameif),
            ),
            CNode::lit("security-level", "Set the ASA security level")
                .kid(CNode::param(ParamKind::Number, "level", "0-100").run(cmd::security_level)),
            // 交换机二层口命令
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
                )
                .kid(
                    CNode::lit("port-security", "Port security")
                        .run(cmd::port_security_enable)
                        .kid(
                            CNode::lit("maximum", "Max secure MAC addresses").kid(
                                CNode::param(ParamKind::Number, "max", "1-1024")
                                    .run(cmd::port_security_max),
                            ),
                        )
                        .kid(
                            CNode::lit("violation", "Violation action").kid(
                                CNode::param(ParamKind::Word, "action", "shutdown | restrict")
                                    .run(cmd::port_security_violation),
                            ),
                        )
                        .kid(CNode::lit("mac-address", "Secure MAC address options").kid(
                            CNode::lit("sticky", "Sticky learning").run(cmd::port_security_sticky),
                        )),
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
            CNode::lit("vrrp", "VRRP virtual router").kid(
                CNode::param(ParamKind::Number, "vrid", "VRRP group, 1-255")
                    .kid(
                        CNode::lit("ip", "Virtual IP address").kid(
                            CNode::param(ParamKind::Ipv4, "vip", "Virtual IP").run(cmd::vrrp_vip),
                        ),
                    )
                    .kid(
                        CNode::lit("priority", "Set the priority").kid(
                            CNode::param(ParamKind::Number, "priority", "1-254")
                                .run(cmd::vrrp_priority),
                        ),
                    )
                    .kid(CNode::lit("preempt", "Enable preemption").run(cmd::vrrp_preempt)),
            ),
            CNode::lit("channel-group", "EtherChannel/aggregation group").kid(
                CNode::param(ParamKind::Number, "group", "Channel group number, 1-128")
                    .run(cmd::lag_bind_ios)
                    .kid(
                        CNode::lit("mode", "EtherChannel mode").kid(
                            CNode::param(ParamKind::Word, "mode", "active | passive | on")
                                .run(cmd::lag_bind_ios),
                        ),
                    ),
            ),
            CNode::lit("storm-control", "Broadcast storm control").kid(
                CNode::lit("broadcast", "Broadcast storm control").kid(
                    CNode::lit("level", "Allowed broadcast rate").kid(
                        CNode::param(ParamKind::Number, "pps", "Broadcast frames per second")
                            .run(cmd::storm_control),
                    ),
                ),
            ),
            CNode::lit("rate-limit", "Committed access rate").kid(
                CNode::lit("input", "Input rate limit").kid(
                    CNode::param(ParamKind::Number, "kbps", "Rate in kbps").run(cmd::rate_limit),
                ),
            ),
            CNode::lit("no", "Negate a command or set its defaults")
                .kid(CNode::lit("shutdown", "Undo shutdown of the interface").run(h_no_shutdown))
                .kid(
                    CNode::lit("ip", "IP information").kid(
                        CNode::lit("address", "Remove the IP address").run(cmd::no_ip_address),
                    ),
                )
                .kid(
                    CNode::lit("channel-group", "Leave the EtherChannel group")
                        .run(cmd::lag_unbind),
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
        line: vec![
            CNode::lit("login", "Enable login authentication")
                .run(cmd::mgmt_login_local)
                .kid(CNode::lit("local", "Use the local user database").run(cmd::mgmt_login_local)),
            CNode::lit("transport", "Line transport settings").kid(
                CNode::lit("input", "Allowed inbound protocols").kid(
                    CNode::param(ParamKind::WordList, "protos", "ssh | telnet | all | none")
                        .run(cmd::mgmt_transport),
                ),
            ),
            CNode::lit("exit", "Exit from line configuration mode").run(h_exit),
        ],
        aaa: vec![CNode::lit("exit", "Exit from AAA configuration mode").run(h_exit)],
        mst: vec![
            CNode::lit("instance", "Map VLANs to an MST instance").kid(
                CNode::param(ParamKind::Number, "id", "MST instance, 1-4094").kid(
                    CNode::lit("vlan", "VLAN list").kid(
                        CNode::param(ParamKind::VlanList, "vlans", "VLAN list, e.g. 10,20-24")
                            .run(cmd::stp_mst_instance_vlan),
                    ),
                ),
            ),
            CNode::lit("name", "Region name")
                .kid(CNode::param(ParamKind::Word, "name", "Region name").run(cmd::stp_mst_noop)),
            CNode::lit("revision", "Region revision").kid(
                CNode::param(ParamKind::Number, "rev", "Revision, 0-65535").run(cmd::stp_mst_noop),
            ),
            CNode::lit("exit", "Exit from MST configuration mode").run(h_exit),
        ],
        ospf: vec![
            CNode::lit("network", "Enable OSPF on a network").kid(
                CNode::param(ParamKind::Ipv4, "net", "Network address").kid(
                    CNode::param(ParamKind::Wildcard, "wildcard", "Wildcard bits").kid(
                        CNode::lit("area", "OSPF area").kid(
                            CNode::param(ParamKind::Number, "area", "Area ID")
                                .run(cmd::ospf_network),
                        ),
                    ),
                ),
            ),
            CNode::lit("exit", "Exit from router configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        ospf_area: vec![
            CNode::lit("exit", "Exit from router configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        bgp: vec![
            CNode::lit("neighbor", "Configure a BGP neighbor").kid(
                CNode::param(ParamKind::Ipv4, "ip", "Neighbor IP address")
                    .kid(CNode::lit("remote-as", "Remote AS number").kid(
                        CNode::param(ParamKind::Number, "asn", "Remote AS").run(cmd::bgp_neighbor),
                    ))
                    .kid(
                        CNode::lit("route-map", "Apply a route-map").kid(
                            CNode::param(ParamKind::Word, "policy", "Route map name").kid(
                                CNode::param(ParamKind::Word, "dir", "in | out")
                                    .run(cmd::bgp_peer_policy),
                            ),
                        ),
                    ),
            ),
            CNode::lit("network", "Advertise a network").kid(
                CNode::param(ParamKind::Ipv4, "net", "Network address")
                    .kid(
                        CNode::lit("mask", "Subnet mask").kid(
                            CNode::param(ParamKind::DottedMask, "mask", "Subnet mask")
                                .run(cmd::bgp_network),
                        ),
                    )
                    .kid(
                        CNode::param(ParamKind::DottedMask, "mask", "Subnet mask")
                            .run(cmd::bgp_network),
                    ),
            ),
            CNode::lit("exit", "Exit from router configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        isis: vec![
            CNode::lit("net", "Network Entity Title").kid(
                CNode::param(ParamKind::Word, "net", "e.g. 49.0001.0000.0000.0001.00")
                    .run(cmd::isis_net),
            ),
            CNode::lit("exit", "Exit from router configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        route_policy: vec![
            CNode::lit("match", "Match values").kid(
                CNode::lit("ip", "IP address").kid(
                    CNode::lit("address", "Match an access list").kid(
                        CNode::param(ParamKind::Number, "acl", "ACL number")
                            .run(cmd::route_policy_match_acl),
                    ),
                ),
            ),
            CNode::lit("set", "Set values").kid(
                CNode::lit("local-preference", "BGP local preference").kid(
                    CNode::param(ParamKind::Number, "pref", "Local preference value")
                        .run(cmd::route_policy_set_local_pref),
                ),
            ),
            CNode::lit("exit", "Exit from route-map configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        zone: vec![
            CNode::lit("exit", "Exit from zone configuration mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        security_policy: vec![
            CNode::lit("exit", "Exit from security policy mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        security_rule: vec![
            CNode::lit("source", "Source zone").kid(
                CNode::param(ParamKind::Word, "src", "Source zone name")
                    .run(cmd::security_rule_source),
            ),
            CNode::lit("destination", "Destination zone").kid(
                CNode::param(ParamKind::Word, "dst", "Destination zone name")
                    .run(cmd::security_rule_destination),
            ),
            CNode::lit("action", "Policy action").kid(
                CNode::param(ParamKind::Word, "action", "permit | deny").run(cmd::security_action),
            ),
            CNode::lit("exit", "Exit from zone-pair mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
        ipsec: vec![
            CNode::lit("local-subnet", "Local protected subnet").kid(
                CNode::param(ParamKind::Ipv4, "net", "Network").kid(
                    CNode::param(ParamKind::PrefixLen, "len", "Prefix length")
                        .run(cmd::ipsec_local_subnet),
                ),
            ),
            CNode::lit("remote-subnet", "Remote protected subnet").kid(
                CNode::param(ParamKind::Ipv4, "net", "Network").kid(
                    CNode::param(ParamKind::PrefixLen, "len", "Prefix length")
                        .run(cmd::ipsec_remote_subnet),
                ),
            ),
            CNode::lit("exit", "Exit from IPsec mode").run(h_exit),
            CNode::lit("end", "Exit to privileged mode").run(h_end),
        ],
    }
}
