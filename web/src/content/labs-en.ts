export interface LabEn {
  title: string;
  brief: string;
  tasks: string[];
  checkLabels: string[];
}

export const LABS_EN: Record<string, LabEn> = {
  'office-l2': {
    title: 'Lab 1 · Layer 2 Office Network (VLAN Isolation)',
    brief:
      'Two switches connected by a trunk. Two PCs in the finance department (VLAN10) reach each other across the switches, while the admin department (VLAN20) stays isolated from them.',
    tasks: [
      'SW1/SW2: vlan batch 10 20 (IOS: vlan 10, vlan 20)',
      'SW1 G0/0/1 to PC1 -> access VLAN10; SW1 G0/0/3 to PC3 -> access VLAN20',
      'SW2 G0/0/2 to PC2 -> access VLAN10',
      'Set the interconnect ports on both switches to trunk and allow VLAN10 (on Huawei/H3C remember allow-pass / permit)',
      'From PC1 ping 192.168.10.12 (should succeed) and ping 192.168.10.13 (should fail)',
    ],
    checkLabels: [
      'PC1 -> PC2 (same VLAN, across switches) reachable',
      'PC1 -> PC3 (cross-VLAN) isolated',
      'SW1 interconnect port trunk allows VLAN10',
      'SW2 interconnect port trunk allows VLAN10',
    ],
  },
  'office-l3': {
    title: 'Lab 2 · Layer 3 Switch Inter-VLAN Routing (SVI)',
    brief:
      'One Layer 3 switch carries two VLANs, with Vlanif10 / Vlanif20 configured as gateways to provide inter-VLAN routing. Experience VLAN isolation combined with Layer 3 reachability.',
    tasks: [
      'SW1: vlan batch 10 20',
      'G0/0/1 -> access VLAN10 (to PC1); G0/0/2 -> access VLAN20 (to PC2)',
      'interface Vlanif10 with 192.168.10.1/24; interface Vlanif20 with 192.168.20.1/24 (IOS: interface vlan 10)',
      'PC1 gateway 192.168.10.1, PC2 gateway 192.168.20.1 (set them in the properties panel)',
      'From PC1 ping 192.168.20.12 — this time it should succeed!',
    ],
    checkLabels: [
      'Vlanif10 = 192.168.10.1',
      'Vlanif20 = 192.168.20.1',
      'PC1 -> PC2 reachable across VLANs',
    ],
  },
  'stp-redundancy': {
    title: 'Lab 3 · Redundant Links and STP',
    brief:
      'Two links between the switches form a loop. Observe how STP blocks one of them automatically and how the topology converges after a failure. BPDUs are visible in packet captures.',
    tasks: [
      'After loading the topology wait 30 seconds (virtual time) for STP to converge, then run show spanning-tree on SW1 to inspect port roles',
      'Set the interconnect ports on both switches to trunk and allow VLAN10 (Huawei: port link-type trunk + allow-pass)',
      'Make SW1 the root bridge: system-view -> stp priority 4096 (IOS: spanning-tree vlan 1 priority 4096)',
      'Start a packet capture on one interconnect link and observe the BPDUs (one every 2 seconds)',
      'On SW1 shut down the primary link port; after convergence PC1 can still ping PC2 — redundancy works!',
      '(Optional) Run stp disable on both switches and feel the broadcast storm: the drop counter in the status bar spikes',
    ],
    checkLabels: [
      'SW1 is the root bridge (priority 4096)',
      'SW2 converged: the root port is on one of the interconnect links',
      'PC1 -> PC2 reachable over the redundant path',
    ],
  },
  enterprise: {
    title: 'Lab 4 · Enterprise Internet Access (DHCP + ACL + NAT)',
    brief:
      'A LAN PC obtains its address automatically through DHCP; the router is configured with NAT to reach the Internet; an ACL blocks the office network from pinging the finance server. A comprehensive IA lab.',
    tasks: [
      'R1: g0/0 (LAN port) 192.168.1.1/24, g0/1 (WAN port) 202.100.1.1/24, both no shutdown',
      'R1: ip dhcp pool LAN -> network 192.168.1.0 255.255.255.0 -> default-router 192.168.1.1',
      'Enable DHCP automatic addressing in the PC1 properties panel -> it should obtain 192.168.1.2',
      'R1: ip nat inside on g0/0, ip nat outside on g0/1; check translations with show ip nat translations',
      '(Optional) access-list 100 deny icmp 192.168.1.0 0.0.0.255 202.100.1.0 0.0.0.255 plus interface application, to see the blocking in action',
      'PC1 ping 202.100.1.100 — it works! Your traffic has been NATed to a public address',
    ],
    checkLabels: [
      'PC1 obtained 192.168.1.2 via DHCP',
      'PC1 -> Internet server reachable (NAT in effect)',
      'R1 has a directly connected Internet subnet',
    ],
  },
  'lag-redundancy': {
    title: 'Lab 5 · Link Aggregation and Bandwidth Redundancy',
    brief:
      'Two links between the switches are bundled into one logical link, Eth-Trunk1, using LACP. STP sees only a single port, both links forward at the same time, and losing one link does not interrupt service.',
    tasks: [
      'SW1/SW2: system-view -> vlan batch 10',
      'SW1 G0/0/1 to PC1 -> port link-type access + port default vlan 10 (the same for SW2 G0/0/3 to PC2)',
      'Interconnect ports (SW1 G0/0/2, G0/0/3; SW2 G0/0/1, G0/0/2) -> port link-type trunk + port trunk allow-pass vlan 10',
      'SW1/SW2: interface Eth-Trunk 1 creates the aggregate port; then enter both member ports and run eth-trunk 1',
      'display eth-trunk 1: both members should be Up; PC1 ping 192.168.10.12',
      'Optional: shut down one member port, then ping again — service stays up, which is bandwidth redundancy',
    ],
    checkLabels: [
      'SW1 member port G0/0/2 has joined aggregation group 1',
      'SW1 member port G0/0/3 has joined aggregation group 1',
      'Both members of SW1 aggregation group 1 are selected (LACP Up)',
      'SW2 member port G0/0/1 has joined aggregation group 1',
      'SW2 member port G0/0/2 has joined aggregation group 1',
      'Both members of SW2 aggregation group 1 are selected (LACP Up)',
      'PC1 -> PC2 reachable over the aggregate link',
    ],
  },
  'mgmt-ssh': {
    title: 'Lab 6 · Device Management Plane Hardening (SSH/Telnet)',
    brief:
      'Configure a local administrator and VTY login on the router, allowing SSH access only. SSH login from the PC succeeds while Telnet is rejected — the first lesson in management plane hardening.',
    tasks: [
      'R1: enable -> configure terminal -> interface g0/0 -> ip address 192.168.1.1 255.255.255.0 -> no shutdown -> exit',
      'R1: username admin privilege 15 secret Cisco@123',
      'R1: line vty 0 4 -> login local -> transport input ssh -> exit',
      'PC1 terminal: ssh 192.168.1.1 -> username admin, password Cisco@123 -> seeing R1# means the login succeeded',
      'After logging in run show ip interface brief to inspect the R1 interfaces, then exit to disconnect',
      'PC1 terminal: telnet 192.168.1.1 — it should report connection refused (VTY allows SSH only)',
    ],
    checkLabels: [
      'R1 has local user admin',
      'R1 VTY allows SSH access only',
      'PC1 SSH login to R1 succeeds',
      'PC1 Telnet login is rejected',
    ],
  },
  'bgp-route-policy': {
    title: 'Lab 7 · BGP Route Policy',
    brief:
      'R1—R2—R3 chain three autonomous systems (AS65001—AS65002—AS65003). R1 advertises two prefixes, one of which (172.16.9.0/24) is a controlled service: a route-policy plus an ACL on R2 inbound filters only that prefix. Watch the filter propagate through BGP — R3 never learns the controlled prefix either, so the policy protects the whole downstream.',
    tasks: [
      'R1: G0/0/0 192.168.1.1/24, G0/0/1 10.0.0.1/24, G0/0/2 172.16.9.1/24, all with undo shutdown',
      'R2: G0/0/0 10.0.0.2/24, G0/0/1 10.0.1.1/24, all with undo shutdown (transit AS, no service subnet)',
      'R3: G0/0/0 10.0.1.2/24, G0/0/1 192.168.3.1/24, all with undo shutdown',
      'R1: bgp 65001 -> peer 10.0.0.2 as-number 65002 -> network 192.168.1.0 255.255.255.0 -> network 172.16.9.0 255.255.255.0',
      'R2: bgp 65002 -> peer 10.0.0.1 as-number 65001 -> peer 10.0.1.2 as-number 65003 -> quit',
      'R3: bgp 65003 -> peer 10.0.1.1 as-number 65002 -> network 192.168.3.0 255.255.255.0',
      'R2: acl 2000 -> rule permit source 172.16.9.0 0.0.0.255 -> quit (match only the controlled prefix)',
      'R2: route-policy FILTER deny node 10 -> if-match acl 2000 -> quit',
      'R2: route-policy FILTER permit node 20 -> quit',
      'R2: bgp 65002 -> peer 10.0.0.1 route-policy FILTER import -> quit',
      'display bgp routing-table: R2 should only have 192.168.1.0/24 and 192.168.3.0/24; R3 should not learn 172.16.9.0/24 either — the R2 filter has propagated downstream (IOS: route-map + match ip address + neighbor ... route-map ... in)',
      'PC2 ping 192.168.1.10 should succeed; ping 172.16.9.10 should fail',
    ],
    checkLabels: [
      'R2 learned 192.168.1.0/24 from R1 (BGP route)',
      'R2 did not learn 172.16.9.0/24 (filtered by policy)',
      'R3 learned 192.168.1.0/24 (routes cross AS boundaries)',
      'R3 also lacks the controlled prefix (filter propagated downstream)',
      'R1 learned 192.168.3.0/24 from R3 (return route propagated)',
      'PC2 -> PC1 (192.168.1.10) reachable',
      'PC2 -> controlled server (172.16.9.10) unreachable',
    ],
  },
  'mpls-l3vpn': {
    title: 'Lab 8 · MPLS L3VPN (VRF + Static LSP)',
    brief:
      'Two customer sites connect across a provider MPLS backbone. PEs isolate customer routes in VRFs, push a transport label plus a VPN label at ingress, P swaps only the outer label, and the egress PE delivers the packet back into the customer VRF by its VPN label.',
    tasks: [
      'PE1: system-view -> ip vpn-instance CUST1 (do not quit) -> interface g0/0/0 -> ip binding vpn-instance CUST1 -> ip address 192.168.1.1 24 -> undo shutdown -> quit',
      'PE1: interface g0/0/1 -> ip address 10.0.0.1 24 -> undo shutdown -> quit',
      'PE1: static-lsp ingress to-ce2 destination 192.168.2.0 24 nexthop 10.0.0.2 out-label 100 vrf CUST1 vpn-label 1001',
      'PE1 return path: static-lsp egress trans400 incoming-label 400 -> static-lsp egress vpn1002 incoming-label 1002 vrf CUST1',
      'P: G0/0/0 10.0.0.2/24, G0/0/1 10.0.1.1/24; static-lsp transit t1 incoming-label 100 nexthop 10.0.1.2 out-label 200',
      'P: static-lsp transit t2 incoming-label 300 nexthop 10.0.0.1 out-label 400',
      'PE2: ip vpn-instance CUST1 -> g0/0/0 10.0.1.2/24, g0/0/1 bound to CUST1 with 192.168.2.1/24',
      'PE2: static-lsp egress trans200 incoming-label 200 -> static-lsp egress vpn1001 incoming-label 1001 vrf CUST1',
      'PE2 return path: static-lsp ingress to-ce1 destination 192.168.1.0 24 nexthop 10.0.1.1 out-label 300 vrf CUST1 vpn-label 1002',
      'display mpls lsp should show VRF=CUST1 and VPN labels; CE1 ping 192.168.2.10 (IOS: mpls static-lsp ... with the same structure)',
    ],
    checkLabels: [
      'PE1 has the outbound VPN LSP to-ce2',
      'PE2 maps VPN label 1001 to the VRF',
      'CE1 -> CE2 reachable across the MPLS backbone',
      'CE2 -> CE1 return path reachable',
    ],
  },
  'campus-capstone': {
    title: 'Lab 9 · Large Campus Capstone (VLAN/LACP/OSPF/VRRP/NAT)',
    brief:
      'Each building has an access switch connected by a two-link LACP bundle; core switch DSW handles inter-VLAN routing; two core routers provide a VRRP gateway and run OSPF, with CORE1 doing NAT to the Internet. A capstone that ties M2 through M5 together.',
    tasks: [
      'SW1: vlan batch 10 20; G0/0/1 access VLAN10 (PC1), G0/0/2 access VLAN20 (PC3); G0/0/3, G0/0/4, G0/0/5 as trunk allowing vlan 10 20',
      'SW2: vlan batch 10 20; G0/0/1, G0/0/2 as trunk allowing vlan 10 20; G0/0/3 access VLAN10 (PC2), G0/0/4 access VLAN20 (PC4)',
      'SW1/SW2: add member ports (SW1 G0/0/3, G0/0/4; SW2 G0/0/1, G0/0/2) with eth-trunk 1; members already carry the trunk config, no extra trunk config on the aggregate (display eth-trunk 1 should show 2 Up)',
      'DSW: vlan batch 10 20 99; G0/0/1 trunk allowing vlan 10 20 (to SW1); G0/0/2, G0/0/3 access VLAN99 (to cores)',
      'DSW: Vlanif10 192.168.10.1/24, Vlanif20 192.168.20.1/24, Vlanif99 10.0.0.2/24; ip route-static 0.0.0.0 0 10.0.0.1 (gateway = VRRP VIP)',
      'CORE1 (IOS): g0/0 10.0.0.10/24 + vrrp 1 ip 10.0.0.1 + priority 200 + preempt + ip nat inside; g0/1 10.1.0.1/30; g0/2 203.0.113.1/24 + ip nat outside',
      'CORE1: ip route 192.168.0.0 255.255.0.0 10.0.0.2; router ospf 1 -> network 10.1.0.0 0.0.0.3 area 0',
      'CORE2 (IOS): g0/0 10.0.0.11/24 + vrrp 1 ip 10.0.0.1; g0/1 10.1.0.2/30; router ospf 1 -> network 10.1.0.0 0.0.0.3 area 0',
      'Verify: display vrrp (CORE1 Master), show ip ospf neighbor (FULL), display eth-trunk 1 (2 Up); PC1 ping PC2/PC3/203.0.113.10',
    ],
    checkLabels: [
      'Both members of SW1 aggregation group 1 are selected',
      'DSW Vlanif10 = 192.168.10.1',
      'DSW Vlanif20 = 192.168.20.1',
      'CORE1 is Master of VRRP group 1',
      'CORE1 has an OSPF neighbor (CORE2)',
      'PC1 -> PC2 (same VLAN, cross-building) reachable',
      'PC1 -> PC3 (cross-VLAN via SVI) reachable',
      'PC1 -> Internet server (NAT) reachable',
    ],
  },
  'firewall-zone': {
    title: 'Lab 10 · Firewall Security Zones and Policy',
    brief:
      'An inside PC reaches an Internet server through a firewall. Put the two interfaces into trust / untrust zones and permit only trust -> untrust traffic; untrusted-initiated access is denied by default while replies are allowed by the stateful session.',
    tasks: [
      'FW1: system-view -> interface g0/0/0 -> ip address 192.168.1.1 24 -> undo shutdown -> quit',
      'FW1: interface g0/0/1 -> ip address 203.0.113.1 24 -> undo shutdown -> quit',
      'FW1: firewall zone trust -> add interface g0/0/0 -> quit (IOS: zone security INSIDE + interface zone-member security INSIDE)',
      'FW1: firewall zone untrust -> add interface g0/0/1 -> quit',
      'FW1: security-policy -> rule name allow-out -> source-zone trust -> destination-zone untrust -> action permit -> quit -> quit',
      'FW1: display firewall zone and display security-policy to check zones and rules',
      'SRV -> PC1 should fail first (untrusted-initiated access is denied); PC1 -> SRV should succeed with the reply allowed by the stateful session',
    ],
    checkLabels: [
      'FW1 G0/0/0 is in the trust zone',
      'FW1 G0/0/1 is in the untrust zone',
      'A trust -> untrust permit policy exists',
      'SRV -> PC1 is denied by default',
      'PC1 -> SRV is permitted (stateful reply)',
    ],
  },
'ipsec-site': {
    title: 'Lab 11 · IPsec Site-to-Site VPN',
    brief:
      'Two branch routers connect across a public internet formed by an ISP router (R1—ISP—R2). Configure IPsec tunnels to encapsulate traffic between 192.168.1.0/24 and 192.168.2.0/24 into public packets. Capture the ISP—R2 link: only outer encapsulated traffic is visible, the private 192.168.x.x subnets are hidden — that is what a VPN is for.',
    tasks: [
      'R1: G0/0/0 192.168.1.1/24, G0/0/1 10.0.0.1/24, both undo shutdown; ISP: G0/0/0 10.0.0.2/24, G0/0/1 10.0.1.1/24, both undo shutdown; R2: G0/0/0 10.0.1.2/24, G0/0/1 192.168.2.1/24, both undo shutdown',
      'Public routes: R1 ip route-static 10.0.1.0 24 10.0.0.2; R2 ip route-static 10.0.0.0 24 10.0.1.1 (a VRP interface quit lands back in system view, so you can continue there). ISP only has its two connected public segments — never configure any private routes on it',
      'R1: ipsec tunnel T1 local 10.0.0.1 remote 10.0.1.2 -> local-subnet 192.168.1.0 24 -> remote-subnet 192.168.2.0 24 -> quit',
      'R2: ipsec tunnel T1 local 10.0.1.2 remote 10.0.0.1 -> local-subnet 192.168.2.0 24 -> remote-subnet 192.168.1.0 24 -> quit',
      'display ipsec tunnel should show both protected subnets; PC1 ping 192.168.2.10 should succeed',
      'Capture the ISP—R2 link: packets are public-to-public encapsulated traffic (10.0.x.x), nothing from 192.168.x.x is visible',
      '(IOS: ipsec tunnel T1 local ... remote ... with the same simplified commands)',
    ],
    checkLabels: [
      'R1 has IPsec tunnel T1',
      'R2 has IPsec tunnel T1',
      'ISP has no private routes (public net cannot reach the subnets)',
      'PC1 -> PC2 reachable through the IPsec tunnel',
      'PC2 -> PC1 return path reachable',
    ],
  },
  'sslvpn-remote': {
    title: 'Lab 12 · SSL VPN Remote Access',
    brief:
      'An outside PC authenticates to an SSL VPN gateway, receives a virtual IP, and then reaches an inside server. The firewall denies outside-initiated inside access by default; the SSL VPN tunnel is the controlled remote-access path.',
    tasks: [
      'FW1: G0/0/0 (outside) 203.0.113.1/24, G0/0/1 (inside) 192.168.1.1/24, both undo shutdown',
      'FW1: firewall zone untrust -> add interface g0/0/0 -> quit; firewall zone trust -> add interface g0/0/1 -> quit',
      'FW1: aaa -> local-user admin password cipher Admin@123 -> quit',
      'FW1: sslvpn virtual-ip 10.10.0.0 24',
      'PC1: sslvpn connect 203.0.113.1 admin Admin@123 — should obtain virtual IP 10.10.0.2',
      'FW1: display sslvpn session should show the virtual IP and the client real IP',
      'PC1 ping 192.168.1.10 — should succeed through the SSL VPN tunnel (fails before connecting)',
    ],
    checkLabels: [
      'PC1 has an established SSL VPN session',
      'PC1 -> inside server reachable through the VPN',
    ],
  },
'wlan-capwap': {
    title: 'Lab 13 · Wireless Campus (AC + AP + CAPWAP + WPA2)',
    brief:
      'The AC manages the AP over CAPWAP: it pushes the SSID, security policy and service VLAN, and the AP bridges wireless clients into the wired VLAN. Only clients that associate with the correct WPA2 passphrase can reach the wired server.',
    tasks: [
      'AC1: interface g0/0/0 -> ip address 10.0.0.2 24 -> undo shutdown -> quit',
      'AC1: wlan ssid CORP -> wlan security wpa2 psk Admin@123 -> wlan vlan 10',
      'SW1: vlan batch 10; G0/0/2 toward the AP as trunk allowing vlan 10; G0/0/3 toward SRV as access VLAN10',
      'AP1: G0/0/1 as trunk allowing vlan 10; interface vlanif 1 -> ip address 10.0.0.10 24 -> quit',
      'AP1: capwap ac 10.0.0.2 — display wlan should show CORP / Joined',
      'PC1: wifi connect CORP Admin@123 -> wifi status; PC2 tries a wrong passphrase (should fail)',
      'PC1 ping 192.168.10.10 should succeed; PC2 is unassociated and its ping should fail',
    ],
    checkLabels: [
      'AP1 registered with AC1',
      'AC1 has SSID CORP',
      'PC1 is associated over Wi-Fi',
      'PC1 -> wired server reachable',
      'PC2 (unassociated) cannot reach the server',
    ],
  },
'wlan-advanced': {
    title: 'Lab 14 · Multi-SSID / WPA3 / Wireless Roaming',
    brief:
      'The AC pushes two SSIDs: CORP (WPA2, VLAN10) and GUEST (WPA3-SAE, VLAN20). Both APs carry both WLANs and clients associate by AP priority; use wifi roam to move between APs without interrupting service, while SSID/VLAN isolation remains.',
    tasks: [
      'AC1: interface g0/0/0 -> ip address 10.0.0.2 24 -> undo shutdown -> quit',
      'AC1: wlan ssid CORP -> wlan security wpa2 psk Admin@123 -> wlan vlan 10',
      'AC1: wlan ssid GUEST -> wlan security wpa3 sae Guest@123 -> wlan vlan 20',
      'SW1: vlan batch 10 20; G0/0/2 and G0/0/3 toward AP1/AP2 as trunk allowing vlan 10 20; G0/0/4 to SRV10 access VLAN10; G0/0/5 to SRV20 access VLAN20',
      'AP1/AP2: G0/0/1 trunk allowing vlan 10 20; Vlanif1 10.0.0.10/24 and 10.0.0.11/24; wlan priority 200/100; capwap ac 10.0.0.2',
      'PC1: wifi connect CORP Admin@123; PC2: wifi connect GUEST Guest@123 (a wrong passphrase should fail)',
      'PC1 ping 192.168.10.10 and PC2 ping 192.168.20.10 should succeed; PC1 ping 192.168.20.10 should fail (SSID/VLAN isolation)',
      'PC1: wifi roam — should move to AP2; pinging 192.168.10.10 should still succeed',
    ],
    checkLabels: [
      'AP1 registered with AC1',
      'AP2 registered with AC1',
      'AC1 has both CORP and GUEST configured',
      'PC1 is associated over Wi-Fi',
      'PC2 is associated over Wi-Fi',
      'PC1 roamed to AP2',
      'PC1 -> CORP server reachable',
      'PC2 -> GUEST server reachable',
      'CORP -> GUEST cross-SSID isolated',
    ],
  },
'vxlan-dc': {
    title: 'Lab 15 · VXLAN Data Center Cross-DC Layer 2',
    brief:
      'Two data-center switches act as VTEPs and map local VLAN10 to VNI 10000 over an L3 underlay, extending Layer 2 across data centers. H1 and H2 are in the same subnet but different sites and can still reach each other directly.',
    tasks: [
      'DSW1: vlan batch 10 100; G0/0/1 to H1 access VLAN10; G0/0/2 to DSW2 access VLAN100',
      'DSW1: interface vlanif 100 -> ip address 10.0.0.1 24 -> undo shutdown',
      'DSW1: vxlan local 10.0.0.1 -> vxlan vni 10000 vlan 10 -> vxlan peer 10.0.0.2',
      'DSW2: vlan batch 10 100; G0/0/1 to DSW1 access VLAN100; G0/0/2 to H2 access VLAN10',
      'DSW2: interface vlanif 100 -> ip address 10.0.0.2 24 -> undo shutdown',
      'DSW2: vxlan local 10.0.0.2 -> vxlan vni 10000 vlan 10 -> vxlan peer 10.0.0.1',
      'display vxlan should show VNI 10000 <-> VLAN10 and the peer VTEP; H1 ping 192.168.10.20 should succeed',
      '(Packet capture shows the VXLAN outer frame with the original Ethernet frame inside)',
    ],
    checkLabels: [
      'DSW1 maps VNI 10000 to VLAN10',
      'DSW2 maps VNI 10000 to VLAN10',
      'H1 -> H2 cross-DC Layer 2 reachable',
    ],
  },
'stack-irf': {
    title: 'Lab 16 · Stacking / IRF (One Logical Switch)',
    brief:
      'Two switches share a stack domain and become one logical switch: member links are treated as internal fabric and are not blocked by STP, so PCs under different members communicate in the same VLAN.',
    tasks: [
      'SW1: stack domain 10 -> stack member 1 -> stack priority 200',
      'SW2: stack domain 10 -> stack member 2 -> stack priority 100',
      'SW1/SW2: vlan batch 10; PC-facing ports access VLAN10',
      'SW1/SW2: interconnect ports as trunk allowing vlan 10',
      'display stack should show domain 10, both members, and SW1 as Master',
      'PC1 ping 192.168.10.12 should succeed (Layer 2 across stack members)',
    ],
    checkLabels: [
      'SW1 is member 1 of stack domain 10',
      'SW2 is member 2 of stack domain 10',
      'PC1 -> PC2 reachable across stack members',
    ],
  },
  'mlag-dc': {
    title: 'Lab 17 · M-LAG Cross-Device Link Aggregation',
    brief:
      'Two DSWs form an M-LAG pair sharing one system MAC. Access switch SW1 dual-homes to both DSWs with LACP, both links forward at once, and the peer-link carries cross-device traffic.',
    tasks: [
      'SW1: vlan batch 10 -> stp disable (dual-homing does not rely on STP)',
      'SW1: G0/0/1 to PC1 access VLAN10; G0/0/2 and G0/0/3 to DSW1/DSW2 as trunk allowing vlan 10',
      'SW1: interface eth-trunk 1 -> mode lacp-static -> add G0/0/2 and G0/0/3',
      'DSW1/DSW2: vlan batch 10; mlag domain 1; G0/0/1 to SW1 trunk allowing vlan 10; G0/0/2 interconnect trunk allowing vlan 10 and mlag peer-link',
      'DSW1/DSW2: interface eth-trunk 1 -> mode lacp-static -> add G0/0/1',
      'DSW2: G0/0/3 to PC2 access VLAN10',
      'display mlag should show domain 1 and the shared System MAC; SW1 display eth-trunk 1 should select 2 members',
      'PC1 ping 192.168.10.12 should succeed (both dual-homed links forward)',
    ],
    checkLabels: [
      'DSW1 has M-LAG domain 1',
      'DSW2 has M-LAG domain 1',
      'SW1 dual-homed LACP selected 2 members',
      'PC1 -> PC2 reachable through M-LAG',
    ],
  },
'evpn-dc': {
    title: 'Lab 18 · EVPN Control Plane (Automatic VTEP Discovery)',
    brief:
      'The two VTEPs no longer use static vxlan peers; they advertise EVPN routes (simplified Type-3) to learn each other’s VTEP and VNI, and the data plane automatically builds the VXLAN tunnel.',
    tasks: [
      'DSW1/DSW2: configure VLAN10/100, underlay SVI 10.0.0.1/2, VXLAN local and VNI 10000 <-> VLAN10 as in Lab 15',
      'DSW1: evpn enable -> evpn peer 10.0.0.2 (do not configure a static vxlan peer)',
      'DSW2: evpn enable -> evpn peer 10.0.0.1',
      'display evpn should show the remote VTEP and VNI 10000 route learned through EVPN',
      'display vxlan: the remote VTEP should appear automatically in Peers',
      'H1 ping 192.168.10.20 should succeed (data plane uses the learned VTEP)',
    ],
    checkLabels: [
      'DSW1 learned an EVPN route from DSW2',
      'DSW2 learned an EVPN route from DSW1',
      'H1 -> H2 reachable through EVPN-discovered VTEPs',
    ],
  },
  'port-isolation': {
    title: 'Lab 20 · MUX VLAN and Port Isolation',
    brief:
      'Isolate ports inside one VLAN: PC1 (separate) cannot reach PC2 (group), but both can reach PC3 (principal). This is the core behavior of MUX VLAN / private VLAN.',
    tasks: [
      'SW1: vlan batch 10; G0/0/1 to PC1, G0/0/2 to PC2, G0/0/3 to PC3, all access VLAN10',
      'SW1: G0/0/1 -> mux-vlan separate; G0/0/2 -> mux-vlan group 1; G0/0/3 -> mux-vlan principal',
      '(Optional) run port-isolate enable group 1 on PC1/PC2 ports and observe the same isolation',
      'Check port roles with display mux-vlan / display port-isolate',
      'PC1 -> PC2 should fail; PC1 -> PC3 and PC2 -> PC3 should succeed',
    ],
    checkLabels: [
      'G0/0/1 is a separate port',
      'G0/0/2 is a group 1 port',
      'G0/0/3 is a principal port',
      'PC1 -> PC2 is isolated',
      'PC1 -> PC3 (principal) is reachable',
    ],
  },
  ospfv3: {
    title: 'Lab 21 · OSPFv3 Dynamic Routing',
    brief:
      'Three routers form a chain with a business subnet at each end. R1 and R3 are not directly connected — R3 learns the R1 subnet through OSPFv3 LSA flooding across the middle, and R2 maintains two neighbors at once. With OSPFv3 enabled, PC1 and PC2 can ping6 across all three routers.',
    tasks: [
      'Interface addresses (all undo shutdown): R1 g0/0/0 2001:db8:1::1/64, g0/0/1 2001:db8:12::1/64; R2 g0/0/0 2001:db8:12::2/64, g0/0/1 2001:db8:23::1/64; R3 g0/0/0 2001:db8:23::2/64, g0/0/1 2001:db8:3::1/64',
      'R1/R2/R3: ospfv3 1 -> router-id 1.1.1.1 / 2.2.2.2 / 3.3.3.3',
      'R1/R2/R3: run ospfv3 1 area 0 on the interconnect and service interfaces',
      'display ospfv3 peer: R1/R3 each 1 neighbor, R2 should have 2 neighbors',
      'display ipv6 routing-table: R3 should show the far subnet 2001:db8:1::/64 marked O — it was never directly connected',
      'PC1: ping6 2001:db8:3::10 should succeed',
    ],
    checkLabels: [
      'R1 forms an OSPFv3 neighbor',
      'R2 keeps two neighbors (flooding relay)',
      'R3 forms an OSPFv3 neighbor',
      'PC1 -> PC2 ping6 across three routers succeeds',
    ],
  },
  'srv6-be': {
    title: 'Lab 22 · SRv6 BE Forwarding',
    brief:
      'PE1—P—PE2 form an IPv6 underlay backbone, each edge attached to a business subnet. Configure a Locator and local SID on the PEs and encapsulate traffic toward the remote SID; packets hop across the backbone and the egress PE decapsulates and delivers them. Note the P router runs plain IPv6 forwarding with no SRv6 at all — that is the essence of SRv6 BE.',
    tasks: [
      'Interface addresses (all undo shutdown): PE1 g0/0/0 2001:db8:1::1/64, g0/0/1 2001:db8:12::1/64; P g0/0/0 2001:db8:12::2/64, g0/0/1 2001:db8:23::1/64; PE2 g0/0/0 2001:db8:23::2/64, g0/0/1 2001:db8:2::1/64',
      'PE1: srv6 locator 2001:db8:100::/64; srv6 sid 2001:db8:100::1. PE2: srv6 locator 2001:db8:200::/64; srv6 sid 2001:db8:200::1',
      'SRv6 routes: PE1 srv6 route 2001:db8:2::/64 sid 2001:db8:200::1; PE2 srv6 route 2001:db8:1::/64 sid 2001:db8:100::1',
      'Underlay static routes (simulate full IGP reachability): PE1 ipv6 route-static 2001:db8:200:: 64 2001:db8:12::2 and 2001:db8:23:: 64 2001:db8:12::2; P 2001:db8:200:: 64 2001:db8:23::2 and 2001:db8:100:: 64 2001:db8:12::1; PE2 ipv6 route-static 2001:db8:100:: 64 2001:db8:23::1 and 2001:db8:12:: 64 2001:db8:23::1',
      'Do not configure any srv6 command on P — it only forwards IPv6. Capture the PE1—P link to watch encapsulated SRv6 packets hop across the backbone',
      'display srv6 should show the locator, local SID, and routes; PC1 ping6 2001:db8:2::10 should succeed',
    ],
    checkLabels: [
      'PE1 has an SRv6 route to the remote business subnet',
      'PE2 has the return SRv6 route',
      'Underlay IPv6 reachable (PE1 -> P)',
      'PC1 -> PC2 succeeds through SRv6 across the backbone',
    ],
  },
  'f5-ltm': {
    title: 'Lab 19 · F5 LTM Load Balancing',
    brief:
      'The F5 load balancer exposes VIP 10.0.0.100:80 to clients with a two-member backend pool. Client TCP connections to the VIP are load-balanced round-robin with destination and return source translation.',
    tasks: [
      'LB1: G0/0/0 client side 10.0.0.1/24; G0/0/1 to SRV1 10.0.1.1/24; G0/0/2 to SRV2 10.0.2.1/24, all undo shutdown',
      'LB1: ltm pool WEB member 10.0.1.10 80',
      'LB1: ltm pool WEB member 10.0.2.11 80',
      'LB1: ltm virtual VS vip 10.0.0.100 80 pool WEB',
      'display ltm should show VS -> WEB and both pool members',
      'PC1: run tcp 10.0.0.100 80 twice; both should report a successful connection',
      'display ltm: two sessions should appear, pointing to different members (round-robin)',
    ],
    checkLabels: [
      'LB1 virtual server VS = 10.0.0.100:80',
      'PC1 first connection to the VIP succeeds',
      'PC1 second connection to the VIP succeeds',
    ],
  },
};
