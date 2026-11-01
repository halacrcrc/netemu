// English translations, part 3: IA upgrades (b18-b22, ip*, rt*, v6*), services, WLAN,
// and IE topics (OSPF, BGP, IS-IS, MPLS, QoS, automation).
// Source of truth: questions.ts. Option order and correct answer positions are preserved.

export interface QuestionEn { q: string; options: string[]; explain: string }

export const QUESTIONS_EN_3: Record<string, QuestionEn> = {
  // ---------- basics (supplement) ----------
  b24: {
    q: 'Which OSI layer provides reliable end-to-end delivery?',
    options: ['Network layer', 'Transport layer', 'Data link layer', 'Session layer'],
    explain: 'The transport layer (TCP/UDP) handles end-to-end communication; TCP adds reliability via handshakes, sequence numbers, acknowledgments, and retransmission.',
  },
  b19: {
    q: 'When a host sends data, what is the correct encapsulation order?',
    options: [
      'Ethernet header → IP header → TCP header → application data',
      'Application data → TCP header → IP header → Ethernet header',
      'Application data → IP header → TCP header → Ethernet header',
      'TCP header → application data → Ethernet header → IP header',
    ],
    explain: 'The sender encapsulates top-down: application data gets a transport header, then an IP header, then an Ethernet header.',
  },
  b20: {
    q: 'What is the main purpose of the TCP three-way handshake?',
    options: ['Encrypt data', 'Synchronize initial sequence numbers and confirm both sides can send and receive', 'Assign IP addresses', 'Select a route'],
    explain: 'SYN / SYN-ACK / ACK exchange initial sequence numbers and confirm bidirectional reachability before data flows.',
  },
  b21: {
    q: 'Compared with TCP, a typical characteristic of UDP is:',
    options: ['Connection-oriented with reliable retransmission', 'Connectionless, low overhead, no delivery guarantee', 'Always arrives in order', 'Requires a three-way handshake'],
    explain: 'UDP is connectionless with a small header and no retransmission, suitable for DNS, voice, and video that tolerate loss.',
  },
  b22: {
    q: 'What is the scope of an ARP request?',
    options: ['The entire Internet', 'The local broadcast domain', 'Outside the local VLAN', 'Only the local host'],
    explain: 'ARP resolves an IP to a MAC by broadcast, and broadcasts do not cross routers, so it stays within one broadcast domain.',
  },

  // ---------- IPv4 ----------
  ip1: {
    q: 'How many usable host addresses does a /26 subnet provide at most?',
    options: ['30', '62', '64', '126'],
    explain: 'A /26 has 6 host bits, 64 addresses total, and 62 usable after removing the network and broadcast addresses.',
  },
  ip2: {
    q: 'Which of the following is a private address range?',
    options: ['172.16.0.0/12', '8.8.8.0/24', '203.0.113.0/24', '224.0.0.0/4'],
    explain: 'Private ranges are 10.0.0.0/8, 172.16.0.0/12, and 192.168.0.0/16; public routing does not forward them.',
  },
  ip3: {
    q: 'What is the network address of the subnet containing 192.168.1.130/25?',
    options: ['192.168.1.0', '192.168.1.128', '192.168.1.130', '192.168.1.255'],
    explain: 'A /25 has a block size of 128, so 130 falls in the 128–255 block and the network address is 192.168.1.128.',
  },
  ip4: {
    q: 'What is the purpose of a host default gateway?',
    options: ['Resolve domain names', 'Hand off off-subnet traffic to a router', 'Assign MAC addresses', 'Encrypt traffic'],
    explain: 'When the destination is outside the local subnet, the host sends the frame to the gateway router for further forwarding.',
  },
  ip5: {
    q: 'What is the main benefit of using /30 on point-to-point links?',
    options: ['More usable addresses', 'Only two usable addresses, saving address space', 'No mask is needed', 'Automatic encryption'],
    explain: 'A /30 has four addresses and exactly two usable after removing network and broadcast, ideal for two endpoints.',
  },

  // ---------- Static routing ----------
  rt1: {
    q: 'If both 10.0.0.0/8 and 10.1.0.0/16 are in the routing table, which matches destination 10.1.2.3?',
    options: ['10.0.0.0/8', '10.1.0.0/16', 'Both forward', 'The default route'],
    explain: 'Longest prefix match wins: /16 is more specific than /8, so 10.1.0.0/16 is selected.',
  },
  rt2: {
    q: 'What is the prefix of the default route?',
    options: ['0.0.0.0/0', '255.255.255.255/32', '127.0.0.0/8', '224.0.0.0/4'],
    explain: '0.0.0.0/0 matches every destination and is commonly used as an egress route when nothing more specific matches.',
  },
  rt3: {
    q: 'Floating static routes switch over based on:',
    options: ['A higher administrative distance/preference', 'A smaller mask', 'A shorter AS_PATH', 'A lower port number'],
    explain: 'The backup static route gets a higher distance, stays out of the table while the primary exists, and takes over when the primary disappears.',
  },
  rt4: {
    q: 'What is the default administrative distance of a connected route on Cisco devices?',
    options: ['0', '1', '110', '200'],
    explain: 'Connected = 0, static = 1, eBGP = 20, OSPF = 110, iBGP = 200; Huawei/H3C use different defaults.',
  },
  rt5: {
    q: 'What must be true for a static route next hop to be usable?',
    options: ['It must be public', 'It must be recursively reachable', 'It must share the interface subnet', 'It must be a loopback'],
    explain: 'The router must resolve the next hop to an egress interface or connected route; otherwise the static route never enters the table.',
  },

  // ---------- IPv6 ----------
  v61: {
    q: 'How long is an IPv6 address?',
    options: ['32 bits', '64 bits', '128 bits', '256 bits'],
    explain: 'IPv6 is 128 bits, written as eight hexadecimal groups, with :: compressing a run of zeros.',
  },
  v62: {
    q: 'What is the prefix of IPv6 link-local addresses?',
    options: ['2000::/3', 'fe80::/10', 'fc00::/7', 'ff00::/8'],
    explain: 'fe80::/10 is link-local, auto-generated per interface and valid only on the local link.',
  },
  v63: {
    q: 'Which IPv6 mechanism replaces ARP for resolving a neighbor MAC?',
    options: ['NDP', 'DHCPv6', 'PIM', 'OSPFv3'],
    explain: 'Neighbor Discovery uses ICMPv6 NS/NA for address resolution, plus RS/RA and redirects.',
  },
  v64: {
    q: 'Which statement about IPv6 broadcast is correct?',
    options: ['It still uses 255.255.255.255', 'IPv6 has no broadcast and uses multicast instead', 'It broadcasts only inside /64', 'It is provided by DHCPv6'],
    explain: 'IPv6 removed broadcast and uses multicast (such as ff02::1 for all nodes) for one-to-many notification.',
  },
  v65: {
    q: 'Why do IPv6 subnets usually use /64?',
    options: ['Addresses would run out otherwise', 'It lets SLAAC generate addresses from the interface identifier', 'For IPv4 compatibility', 'To reduce multicast'],
    explain: '/64 leaves 64 bits for the interface identifier, so hosts can combine the RA prefix with SLAAC.',
  },

  // ---------- Network services ----------
  sv1: {
    q: 'Why should production use SSH instead of Telnet?',
    options: ['SSH is faster', 'SSH encrypts traffic while Telnet is cleartext', 'Telnet has no password', 'SSH needs no IP address'],
    explain: 'Telnet sends passwords and configuration in cleartext where a capture can read them; SSH provides encryption and integrity.',
  },
  sv2: {
    q: 'What is the main purpose of NTP?',
    options: ['Assign IP addresses', 'Synchronize device clocks', 'Resolve domain names', 'Encrypt routing protocols'],
    explain: 'NTP keeps clocks consistent so logs, certificates, audits, and event correlation have reliable timestamps.',
  },
  sv3: {
    q: 'What does Syslog severity 0 mean?',
    options: ['Debug', 'Emergency', 'Warning', 'Notice'],
    explain: 'Severity 0 is Emergency (most severe); 7 is Debug (most verbose).',
  },
  sv4: {
    q: 'What is the main enhancement of SNMPv3 over SNMPv2c?',
    options: ['Faster polling', 'Authentication and encryption', 'Support for more interfaces', 'Automatic configuration'],
    explain: 'SNMPv3 adds user authentication and encryption; v1/v2c communities are effectively cleartext passwords.',
  },
  sv5: {
    q: 'What is a typical characteristic of TFTP?',
    options: ['TCP-based with strong authentication', 'UDP-based, simple, and unauthenticated', 'Only for web pages', 'Automatically encrypted'],
    explain: 'TFTP runs over UDP, is simple and unauthenticated, and suits configuration backup and image transfer in controlled networks.',
  },

  // ---------- WLAN ----------
  wl1: {
    q: 'Which channels are typically non-overlapping in the 2.4 GHz band?',
    options: ['1, 6, 11', '1, 2, 3', '36, 40, 44', 'All channels are non-overlapping'],
    explain: '2.4 GHz has only three non-overlapping channels (1/6/11); co-channel and adjacent-channel interference sharply reduce airtime efficiency.',
  },
  wl2: {
    q: 'What does SSID identify?',
    options: ['The AP MAC address', 'The wireless network name', 'The encryption algorithm', 'The channel number'],
    explain: 'SSID is the wireless network name; BSSID is the AP radio MAC identifier.',
  },
  wl3: {
    q: 'What is the role of CAPWAP?',
    options: ['Assign public addresses', 'Build control/data tunnels between AP and AC', 'Encrypt wired traffic', 'Elect the STP root bridge'],
    explain: 'CAPWAP carries configuration, heartbeats, and data tunnels between AP and AC, the foundation of centralized wireless management.',
  },
  wl4: {
    q: 'What is the main advantage of WPA3-SAE over WPA2-PSK?',
    options: ['No passphrase needed', 'Resistance to offline dictionary attacks', 'Only supports 2.4 GHz', 'No encryption'],
    explain: 'SAE performs simultaneous authentication of equals, making offline passphrase guessing from a captured handshake impractical.',
  },
  wl5: {
    q: 'Which mechanism do wireless clients use to contend for the channel?',
    options: ['CSMA/CD', 'CSMA/CA', 'Token Ring', 'Polling'],
    explain: 'Wireless is a shared half-duplex medium where collisions cannot be detected while transmitting, so CSMA/CA listens first and avoids collisions.',
  },

  // ---------- OSPF ----------
  os1: {
    q: 'In multi-area OSPF, every area must connect to:',
    options: ['Area 1', 'Backbone area 0', 'Any area', 'Outside the AS'],
    explain: 'Area 0 is the backbone; inter-area routes transit it, and disconnected areas need special handling such as virtual links.',
  },
  os2: {
    q: 'Who generates Type-3 Summary LSAs?',
    options: ['DR', 'ABR', 'ASBR', 'Any router'],
    explain: 'An ABR summarizes a prefix from one area into a Type-3 LSA and advertises it into other areas.',
  },
  os3: {
    q: 'Which LSA does an ASBR generate for redistributed external routes?',
    options: ['Type-1', 'Type-2', 'Type-5', 'Type-3'],
    explain: 'An ASBR describes external routes with Type-5 External LSAs; inside an NSSA it uses Type-7.',
  },
  os4: {
    q: 'Which LSAs does a Stub area reject?',
    options: ['Type-1', 'Type-2', 'Type-3', 'Type-4/5'],
    explain: 'A Stub area rejects Type-4/5 external LSAs and relies on a default route from the ABR.',
  },
  os5: {
    q: 'What does OSPF neighbor state Full mean?',
    options: ['Just received a Hello', 'Databases are synchronized and adjacency is complete', 'Routes were deleted', 'The area mismatches'],
    explain: 'Full means both LSDBs are synchronized and the adjacency is complete, ready for route computation.',
  },

  // ---------- BGP ----------
  bg1: {
    q: 'How is LOCAL_PREF compared during BGP best-path selection?',
    options: ['Lower wins', 'Higher wins', 'It is not used', 'Only AS_PATH matters'],
    explain: 'Higher LOCAL_PREF wins; it selects the egress inside an AS and is usually set by a route-policy.',
  },
  bg2: {
    q: 'What is the role of the AS_PATH attribute?',
    options: ['Records traversed ASes for loop prevention and selection', 'Encrypts routes', 'Allocates ports', 'Synchronizes clocks'],
    explain: 'AS_PATH records the AS sequence, shorter is preferred, and a route containing the local AS is discarded to prevent loops.',
  },
  bg3: {
    q: 'What does MED mainly influence?',
    options: ['The local AS egress', 'How a neighbor AS enters this AS', 'The TCP window', 'The STP root bridge'],
    explain: 'MED suggests an ingress point to a neighbor, influencing inbound traffic; LOCAL_PREF influences local egress selection.',
  },
  bg4: {
    q: 'What core problem does an iBGP route reflector (RR) solve?',
    options: ['Too many iBGP full-mesh sessions', 'eBGP cannot be encrypted', 'BGP does not support IPv6', 'The routing table is too small'],
    explain: 'An RR reflects routes to clients, breaking iBGP split horizon and avoiding N² full-mesh sessions.',
  },
  bg5: {
    q: 'In Cisco default best-path selection, how do eBGP and iBGP routes compare?',
    options: ['iBGP wins', 'eBGP wins', 'They are identical', 'Only MED matters'],
    explain: 'After AS_PATH, Origin, and MED comparisons, eBGP is preferred over iBGP (Cisco AD is also 20 vs 200).',
  },

  // ---------- IS-IS ----------
  is1: {
    q: 'Which part of an IS-IS NET uniquely identifies the router?',
    options: ['Area address', 'System ID', 'SEL', 'Port number'],
    explain: 'A NET consists of area address + System ID + SEL; the System ID uniquely identifies a router.',
  },
  is2: {
    q: 'What is IS-IS Level 2 mainly used for?',
    options: ['Intra-area routing', 'Backbone/inter-area routing', 'Host access', 'Multicast'],
    explain: 'L1 handles the area interior, L2 forms the backbone and carries inter-area routes, and L1-2 routers connect both.',
  },
  is3: {
    q: 'What is the key difference between an IS-IS DIS and an OSPF DR?',
    options: ['DIS has a backup', 'DIS has no backup and can be preempted', 'DIS is not elected', 'DIS only handles IPv6'],
    explain: 'IS-IS has no backup DIS and a new router may preempt it; OSPF has a BDR and the DR does not preempt.',
  },
  is4: {
    q: 'What is a Node SID in Segment Routing?',
    options: ['A segment identifying a router/prefix', 'An encryption label', 'A MAC assignment', 'A clock source'],
    explain: 'A Node SID (Prefix SID) assigns a segment to a node or prefix, and the source encodes the path as a segment list.',
  },
  is5: {
    q: 'An SRv6 SID is fundamentally:',
    options: ['An MPLS label', 'An IPv6 address (Locator + Function)', 'A MAC address', 'A VLAN ID'],
    explain: 'SRv6 encodes the segment identifier as an IPv6 address made of a Locator and a Function, removing the need for MPLS labels.',
  },

  // ---------- MPLS VPN ----------
  mp1: {
    q: 'How many labels does an ingress PE typically push for L3VPN?',
    options: ['One', 'Two (transport + VPN label)', 'Three', 'None'],
    explain: 'The outer transport label lets P routers forward hop by hop; the inner VPN label tells the egress PE which VRF to use.',
  },
  mp2: {
    q: 'What is the main role of RD?',
    options: ['Decide route import/export', 'Make identical prefixes in different VRFs unique in MP-BGP', 'Encrypt packets', 'Allocate labels'],
    explain: 'RD is prepended to a prefix for uniqueness; import/export policy is determined by RT.',
  },
  mp3: {
    q: 'What is the role of RT (Route Target)?',
    options: ['Uniquely identify a prefix', 'Control VRF route import/export', 'Assign IP addresses', 'Compute SPF'],
    explain: 'RT is an extended community; a VRF matches import/export RTs to decide which VPN routes it accepts or publishes.',
  },
  mp4: {
    q: 'What does PHP (penultimate hop popping) mean?',
    options: ['Push a label at ingress', 'The penultimate hop pops the outer label so the last hop forwards directly', 'Re-push a label at egress', 'Drop all labels'],
    explain: 'PHP lets the penultimate hop pop the transport label so the egress router delivers by destination IP or the inner label, reducing last-hop work.',
  },
  mp5: {
    q: 'In inter-AS L3VPN Option C, between which devices are VPNv4 routes exchanged?',
    options: ['Between ASBRs', 'Between PEs using multi-hop MP-eBGP', 'Between CEs', 'Between P routers'],
    explain: 'Option C has PEs exchange VPNv4 directly while ASBRs only forward labeled packets, giving the best scalability.',
  },

  // ---------- QoS and multicast ----------
  qs1: {
    q: 'Which DSCP value is commonly used for voice?',
    options: ['EF (46)', 'CS1 (8)', 'AF11 (10)', 'BE (0)'],
    explain: 'EF (Expedited Forwarding, 46) is used for low-latency, low-jitter voice traffic.',
  },
  qs2: {
    q: 'What is the main difference between policing and shaping?',
    options: ['Policing buffers; shaping drops', 'Policing drops/downgrades excess, shaping buffers and smooths it', 'They are identical', 'Shaping is only for voice'],
    explain: 'Policing enforces a rate by dropping or downgrading excess; shaping buffers excess and sends it smoothly, adding delay.',
  },
  qs3: {
    q: 'Which queue best suits voice traffic?',
    options: ['Low-latency queue (LLQ/priority queue)', 'Best-effort queue', 'Broadcast queue', 'Management queue'],
    explain: 'Voice is sensitive to delay and jitter, so it belongs in an LLQ/priority queue that is scheduled first under congestion.',
  },
  qs4: {
    q: 'What does WRED do?',
    options: ['Randomly drops some packets early to avoid global synchronization', 'Encrypts queues', 'Adds bandwidth', 'Elects a root bridge'],
    explain: 'WRED probabilistically drops low-priority packets as a queue fills, preventing all TCP flows from backing off at once.',
  },
  qs5: {
    q: 'What does the RPF check require for multicast forwarding?',
    options: ['Packets arrive on the shortest-path interface toward the source', 'Packets arrive on any interface', 'Only the destination is checked', 'Only TTL is checked'],
    explain: 'RPF requires multicast packets to arrive on the interface on the shortest path back to the source, otherwise they are dropped to prevent loops.',
  },

  // ---------- Automation and SDN ----------
  au1: {
    q: 'NETCONF typically uses which transport and data format?',
    options: ['SSH + XML', 'HTTP + JSON', 'FTP + YAML', 'Telnet + text'],
    explain: 'NETCONF runs over SSH and encodes data in XML, supporting candidate/commit and rollback.',
  },
  au2: {
    q: 'What is a key characteristic of RESTCONF?',
    options: ['Maps YANG models to RESTful APIs, commonly JSON', 'Only carries binary', 'No HTTPS support', 'Read-only'],
    explain: 'RESTCONF uses HTTP methods to manipulate YANG data, is JSON-friendly, and fits modern automation platforms.',
  },
  au3: {
    q: 'What advantage does Telemetry have over SNMP polling?',
    options: ['Devices push data with finer sampling', 'It needs no network', 'It only collects alarms', 'It requires manual triggers'],
    explain: 'Telemetry lets devices stream metrics continuously, reaching second-level granularity instead of minute-level polling, suiting large-scale monitoring.',
  },
  au4: {
    q: 'What is typical of Ansible?',
    options: ['Requires an agent on devices', 'Agentless, using YAML playbooks to describe the desired state', 'Only manages Windows', 'Not idempotent'],
    explain: 'Ansible connects over SSH/NETCONF without an agent; playbooks idempotently drive devices toward the desired state.',
  },
  au5: {
    q: 'What is the core idea of SDN?',
    options: ['Centralize the control plane in a controller while devices focus on forwarding', 'Remove all routing protocols', 'Use only Layer 2 switching', 'Merge all devices into one'],
    explain: 'SDN moves the control plane into a controller that pushes forwarding policy over southbound interfaces, making devices programmable forwarders.',
  },
};
