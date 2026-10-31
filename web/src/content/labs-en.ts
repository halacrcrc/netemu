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
};
