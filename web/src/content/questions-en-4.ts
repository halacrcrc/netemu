// English translations, part 4: official-outline gaps (OSPF basics, LAG/stack, ops,
// IS-IS basics, access security, reliability, multicast, campus, SD-WAN, AIOps).
// Source of truth: questions.ts. Option order and correct answer positions are preserved.

export interface QuestionEn { q: string; options: string[]; explain: string }

export const QUESTIONS_EN_4: Record<string, QuestionEn> = {
  // ---------- OSPF basics ----------
  os6: {
    q: 'What are the default OSPF Hello and dead intervals?',
    options: ['1s / 3s', '10s / 40s', '30s / 120s', '60s / 180s'],
    explain: 'Broadcast networks default to Hello 10 seconds and dead 40 seconds; both ends must match or the adjacency fails.',
  },
  os7: {
    q: 'How is the OSPF DR elected on a broadcast segment?',
    options: ['Highest interface IP', 'Highest priority, ties broken by Router ID', 'Lowest MAC', 'Highest bandwidth'],
    explain: 'DR priority wins first (higher is better), then Router ID; DR election is not preemptive.',
  },
  os8: {
    q: 'What does the OSPF wildcard 0.0.0.3 match?',
    options: ['A /30 subnet', 'A /24 subnet', 'A single host', 'Every address'],
    explain: 'In a wildcard, 0 means must match and 1 means ignore; 0.0.0.3 allows the last two bits to vary, matching a /30.',
  },
  os9: {
    q: 'What is the OSPF interface cost based on by default?',
    options: ['Interface bandwidth', 'Hop count', 'MAC address', 'Queue length'],
    explain: 'Default cost derives from interface bandwidth (reference bandwidth / bandwidth) and can be overridden with ip ospf cost or ospf cost.',
  },
  os10: {
    q: 'In single-area OSPF, participating interfaces belong to:',
    options: ['Any different areas', 'One area (usually area 0)', 'Only area 1', 'No area at all'],
    explain: 'Single-area deployments put all interfaces in one area (usually backbone area 0), where routers exchange Type-1/2 LSAs.',
  },

  // ---------- LAG and stacking ----------
  lg1: {
    q: 'What happens if both ends of LACP are configured passive?',
    options: ['The bundle comes up', 'The bundle cannot negotiate', 'It becomes static automatically', 'Only STP runs'],
    explain: 'At least one side must be active to send LACPDUs; two passive ends never initiate negotiation, so the bundle never forms.',
  },
  lg2: {
    q: 'After link aggregation, what does STP see?',
    options: ['One port per member link', 'One logical port', 'No ports', 'Two root ports'],
    explain: 'The bundle is one logical port, so member links are never blocked individually and both bandwidth and redundancy remain.',
  },
  lg3: {
    q: 'How is traffic usually load-shared across an aggregate?',
    options: ['Round robin per packet', 'Per-flow hashing (MAC/IP/port)', 'Fixed by VLAN', 'By TTL'],
    explain: 'Hashing spreads flows across members while a single flow normally stays on one link to avoid reordering.',
  },
  lg4: {
    q: 'What decides the Master in a stack?',
    options: ['Lowest MAC', 'Priority and member ID', 'Interface count', 'Latest boot'],
    explain: 'Stacking compares priority (higher wins), then member ID/MAC; the Master manages configuration and forwarding.',
  },
  lg5: {
    q: 'What risk arises when all stack links fail?',
    options: ['Automatic recovery', 'Split-brain (dual master)', 'VLANs disappear', 'OSPF speeds up'],
    explain: 'After a stack split, both members may believe they are Master; real devices use MAD detection to suppress the duplicate.',
  },

  // ---------- Ops and automation basics ----------
  op1: {
    q: 'What does ping primarily verify?',
    options: ['End-to-end IP reachability and round-trip time', 'The routing LSDB', 'VLAN configuration', 'Optical power'],
    explain: 'ping uses ICMP Echo/Reply to verify reachability and RTT, the first step in troubleshooting.',
  },
  op2: {
    q: 'What is traceroute mainly used for?',
    options: ['Viewing the hop-by-hop path and loss point', 'Assigning addresses', 'Synchronizing clocks', 'Encrypting traffic'],
    explain: 'traceroute raises TTL hop by hop and collects ICMP time-exceeded messages to locate the path and failure point.',
  },
  op3: {
    q: 'What is one of the most important preparations before a network change?',
    options: ['Back up configuration and prepare rollback', 'Disable all logs', 'Delete routes', 'Disable STP'],
    explain: 'Back up configuration, define rollback steps, and schedule a maintenance window so problems can be recovered quickly.',
  },
  op4: {
    q: 'Why change only one variable at a time during troubleshooting?',
    options: ['To configure faster', 'So you can tell which change fixed or caused the issue', 'To reduce commands', 'To bypass permissions'],
    explain: 'Changing several variables at once confuses cause and effect; single-variable changes with immediate verification locate the real cause.',
  },
  op5: {
    q: 'What order does structured troubleshooting recommend?',
    options: ['Application → policy → L3 → L2 → physical', 'Physical → L2 → L3 → policy → application', 'Random attempts', 'Logs only'],
    explain: 'Work bottom-up: rule out physical links/interfaces first, then L2, L3, policy, and application, narrowing the scope layer by layer.',
  },

  // ---------- IS-IS basics ----------
  is6: {
    q: 'What does the System ID in an IS-IS NET identify?',
    options: ['The area', 'A single router uniquely', 'A VLAN', 'A process ID'],
    explain: 'A NET = area address + System ID + SEL; the System ID uniquely identifies a router in the domain.',
  },
  is7: {
    q: 'What does an IS-IS Level 1 router mainly maintain?',
    options: ['Link state inside the area', 'Link state of the whole AS', 'Only IPv6 routes', 'Multicast groups'],
    explain: 'L1 handles the area interior; L2 forms the backbone and carries inter-area routes, with L1-2 routers in both.',
  },
  is8: {
    q: 'What is the key difference between the IS-IS DIS and the OSPF DR?',
    options: ['DIS has a backup', 'DIS has no backup and can be preempted', 'DIS is not elected', 'DIS only handles IPv6'],
    explain: 'IS-IS has no backup DIS and a new router may preempt; OSPF has a BDR and the DR does not preempt.',
  },
  is9: {
    q: 'IS-IS packets are encapsulated over:',
    options: ['IP (protocol 89)', 'Layer 2 directly', 'TCP', 'UDP'],
    explain: 'IS-IS runs directly over Layer 2 and does not depend on IP; OSPF is the one encapsulated over IP.',
  },
  is10: {
    q: 'Which command shows IS-IS neighbors on Huawei devices?',
    options: ['display isis peer', 'show isis neighbor', 'display ospf peer', 'display bgp peer'],
    explain: 'VRP uses display isis peer and Cisco uses show isis neighbors; both show adjacency state.',
  },

  // ---------- Access security ----------
  sv6: {
    q: 'What does AAA stand for?',
    options: ['Authentication, Authorization, Accounting', 'Address, Area, Application', 'Audit, Alarm, Archive', 'Access, Aggregation, Authentication'],
    explain: 'AAA = authentication (who), authorization (what they may do), and accounting (what they did).',
  },
  sv7: {
    q: 'Before 802.1X authentication succeeds, a port usually permits only:',
    options: ['All traffic', 'EAP/DHCP/DNS and other authentication-essential traffic', 'Video only', 'Multicast only'],
    explain: 'Unauthenticated endpoints may only send authentication and addressing traffic; business traffic opens after authentication via dynamic VLAN/ACL.',
  },
  sv8: {
    q: 'What does port security mainly do?',
    options: ['Limits learned MAC addresses per port and handles violations', 'Encrypts all packets', 'Assigns IP addresses', 'Elects the root bridge'],
    explain: 'Port security limits MAC addresses and can restrict or shut down on violation, preventing rogue switches and MAC spoofing.',
  },
  sv9: {
    q: 'DHCP Snooping trusted ports should point to:',
    options: ['All access ports', 'Ports facing the legitimate DHCP server', 'Any port', 'Only PC ports'],
    explain: 'Only server-facing ports are trusted; Offers/Acks from untrusted ports are dropped to prevent rogue DHCP servers.',
  },
  sv10: {
    q: 'What does IPSG check source IPs against?',
    options: ['The routing table', 'The DHCP Snooping binding table', 'The ARP table', 'ACL counters'],
    explain: 'IPSG validates the source IP against the DHCP Snooping binding (MAC/IP/VLAN/port), blocking address spoofing.',
  },

  // ---------- Network reliability ----------
  rl1: {
    q: 'What is the core purpose of BFD?',
    options: ['Fast link/neighbor failure detection', 'Address assignment', 'Packet encryption', 'SPF computation'],
    explain: 'BFD uses a lightweight heartbeat to detect failures in milliseconds and notifies OSPF/BGP/VRRP for fast convergence.',
  },
  rl2: {
    q: 'What is the main benefit of binding VRRP to BFD?',
    options: ['Shorter master/backup failover time', 'More VLANs', 'Fewer IP addresses', 'Disabling STP'],
    explain: 'BFD detects failures quickly and notifies VRRP, reducing failover from seconds to hundreds of milliseconds.',
  },
  rl3: {
    q: 'Link aggregation belongs to which layer of redundancy?',
    options: ['Link level', 'Device level', 'Gateway level', 'Application level'],
    explain: 'Link aggregation bundles physical links into one logical link, providing link-level redundancy and bandwidth.',
  },
  rl4: {
    q: 'What does FRR/TI-LFA do?',
    options: ['Switches to a precomputed backup path after failure', 'Assigns multicast addresses', 'Encrypts tunnels', 'Synchronizes clocks'],
    explain: 'FRR/TI-LFA precomputes loop-free backup paths and switches in sub-second time after a link failure.',
  },
  rl5: {
    q: 'M-LAG/vPC belong to which reliability category?',
    options: ['Cross-device link aggregation (device-level redundancy)', 'Routing protocol', 'NAT', 'Multicast'],
    explain: 'M-LAG/vPC share a system identity across two devices and dual-home access, providing device-level redundancy and cross-device aggregation.',
  },

  // ---------- IP multicast ----------
  mc1: {
    q: 'What is the IPv4 multicast address range?',
    options: ['224.0.0.0/4', '10.0.0.0/8', '192.168.0.0/16', '240.0.0.0/4'],
    explain: '224.0.0.0/4 covers IPv4 multicast; 224.0.0.0/24 is link-local protocol multicast.',
  },
  mc2: {
    q: 'What is the main role of IGMP?',
    options: ['Hosts ask routers to join/leave a multicast group', 'Compute unicast routes', 'Assign MAC addresses', 'Encrypt multicast'],
    explain: 'IGMP manages group membership so routers know which interfaces need the group traffic.',
  },
  mc3: {
    q: 'What is the root of the PIM-SM shared tree?',
    options: ['The source router', 'The RP (rendezvous point)', 'The DR', 'The ABR'],
    explain: 'PIM-SM builds a shared tree (*,G) rooted at the RP and switches to the source tree (S,G) as traffic grows.',
  },
  mc4: {
    q: 'The RPF check requires multicast packets to:',
    options: ['Arrive on the shortest-path interface toward the source', 'Arrive on any interface', 'Only check the destination', 'Only check TTL'],
    explain: 'RPF requires packets to arrive on the interface on the shortest path back to the source, otherwise they are dropped to prevent loops.',
  },
  mc5: {
    q: 'What is characteristic of PIM-SSM?',
    options: ['Specify source and group directly, no RP needed', 'Must use a shared tree', 'Supports broadcast only', 'No IGMP needed'],
    explain: 'SSM lets receivers specify (S,G), skipping the RP and shared tree; it suits live streaming with a known source.',
  },

  // ---------- Campus design ----------
  cp1: {
    q: 'What is the typical three-tier campus architecture?',
    options: ['Access-aggregation-core', 'Access-egress-cloud', 'Core-core-core', 'AP-AC-cloud'],
    explain: 'Access connects endpoints, aggregation provides gateways and policy, and core provides high-speed forwarding and egress interconnect.',
  },
  cp2: {
    q: 'Which technology is commonly used for campus gateway redundancy?',
    options: ['VRRP/HSRP', 'DHCP', 'NTP', 'SNMP'],
    explain: 'VRRP/HSRP let two gateways share a virtual IP so the backup takes over seamlessly when the master fails.',
  },
  cp3: {
    q: 'What is the most common wired admission method in campuses?',
    options: ['802.1X', 'WPA2', 'NAT', 'OSPF'],
    explain: '802.1X performs identity authentication at the port and can combine with dynamic VLAN/ACL, the mainstream wired admission method.',
  },
  cp4: {
    q: 'What is the core idea of "policy following the user"?',
    options: ['Policy binds to user identity and follows the user', 'Policy binds to the switch port', 'Policy binds to the IP address', 'Only MAC binding'],
    explain: 'Policy follows users/groups, so it stays with them when they move or change IP addresses.',
  },
  cp5: {
    q: 'Which component centrally manages Huawei CloudCampus?',
    options: ['iMaster NCE', 'F5 LTM', 'ASA', 'WLC'],
    explain: 'iMaster NCE centrally manages campus switches and APs/ACs with templated, cloud-based configuration.',
  },

  // ---------- SD-WAN and transport ----------
  sw1: {
    q: 'What tunnels are commonly used for the SD-WAN overlay?',
    options: ['GRE/IPsec', 'STP', 'IGMP', 'ARP'],
    explain: 'SD-WAN uses GRE/IPsec to build encrypted overlays over multiple underlays, decoupling topology from transport.',
  },
  sw2: {
    q: 'What usually happens to the data plane if the SD-WAN controller fails?',
    options: ['It keeps forwarding with existing policy', 'The network goes down immediately', 'All tunnels close', 'The routing table is cleared'],
    explain: 'Mature designs separate control and data planes, so the data plane keeps forwarding with the policy already installed.',
  },
  sw3: {
    q: 'An SRv6 SID is:',
    options: ['An IPv6 address (Locator + Function)', 'An MPLS label', 'A VLAN ID', 'A MAC address'],
    explain: 'SRv6 encodes the segment identifier as an IPv6 address made of a Locator and a Function.',
  },
  sw4: {
    q: 'What is the main use of SRv6 Policy?',
    options: ['Explicit paths and traffic engineering', 'Address assignment', 'Clock synchronization', 'DR election'],
    explain: 'SRv6 Policy uses a segment list to specify an explicit path, enabling traffic engineering and service guarantees.',
  },
  sw5: {
    q: 'What is the goal of network slicing?',
    options: ['Different services get isolated resources and paths without contention', 'Merge all services', 'Remove QoS', 'Keep only voice'],
    explain: 'Network slicing partitions resources and paths per service to guarantee bandwidth and latency for critical traffic.',
  },

  // ---------- AIOps and troubleshooting ----------
  ai1: {
    q: 'What advantage does Telemetry have over SNMP polling?',
    options: ['Devices push data with finer sampling', 'It needs no network', 'It only collects alarms', 'It requires manual triggers'],
    explain: 'Telemetry streams metrics continuously at second-level granularity, suiting large-scale real-time monitoring.',
  },
  ai2: {
    q: 'What is the core value of AIOps?',
    options: ['Anomaly detection and root-cause correlation, collapsing alarms', 'Replacing all routing protocols', 'Automatic IP assignment', 'Traffic encryption'],
    explain: 'AIOps learns baselines, detects anomalies, and correlates root causes, turning alarm storms into actionable events.',
  },
  ai3: {
    q: 'What is the role of a digital twin in network changes?',
    options: ['Simulate and validate impact before the change', 'Replace real devices', 'Assign VLANs', 'Encrypt configuration'],
    explain: 'A digital twin replicates the network model so change impact can be simulated before rollout, reducing cutover risk.',
  },
  ai4: {
    q: 'What is the most important preparation before a network cutover?',
    options: ['A rollback plan and maintenance window', 'Disable logs', 'Delete backups', 'Disable monitoring'],
    explain: 'Prepare rollback configuration, a maintenance window, and verification steps so service can be restored quickly if something fails.',
  },
  ai5: {
    q: 'Why restore service first during troubleshooting?',
    options: ['To shorten the outage, then find the root cause calmly', 'To hide the problem', 'Root cause is unnecessary', 'To skip verification'],
    explain: 'Stop the bleeding first, then analyze the root cause and add monitoring/automation checks; this is the operations priority rule.',
  },
};
