// English translations, part 1: basics (b*), VLAN (v*), STP (s*).
// Source of truth: questions.ts. Option order and correct answer positions are preserved.

export interface QuestionEn { q: string; options: string[]; explain: string }

export const QUESTIONS_EN_1: Record<string, QuestionEn> = {
  // ---------- basics ----------
  b1: {
    q: 'What does a switch use to forward Ethernet frames?',
    options: ['Destination IP address', 'Destination MAC address', 'Port number', 'TTL value'],
    explain: 'An Ethernet switch is a Layer 2 device: it looks up the destination MAC address in the frame header in its MAC address table and forwards accordingly.',
  },
  b2: {
    q: 'What is the purpose of ARP?',
    options: [
      'Resolve a domain name to an IP address',
      'Resolve an IP address to a MAC address',
      'Resolve a MAC address to an IP address',
      'Dynamically assign IP addresses',
    ],
    explain: 'ARP (Address Resolution Protocol) broadcasts a request asking which device holds a given IP address, then learns the matching MAC address.',
  },
  b3: {
    q: 'What happens to the TTL field of an ICMP echo request as it passes through each router?',
    options: ['It increases by 1', 'It decreases by 1', 'It stays the same', 'It is reset to zero'],
    explain: 'Each router decrements the TTL by 1 while forwarding; at 0 the packet is dropped and a time exceeded message is returned, which is exactly how traceroute works.',
  },
  b4: {
    q: 'For 192.168.1.100/26, what are the subnet mask and the number of usable hosts?',
    options: [
      '255.255.255.192, 62 hosts',
      '255.255.255.224, 30 hosts',
      '255.255.255.192, 64 hosts',
      '255.255.255.240, 14 hosts',
    ],
    explain: '/26 means mask 255.255.255.192 with 6 host bits, so 2^6-2 = 62 usable hosts after removing the network and broadcast addresses.',
  },
  b5: {
    q: 'What does a switch do with a frame whose destination MAC is unknown?',
    options: ['Drops it', 'Floods it to all ports except the ingress port', 'Sends back an error message', 'Buffers it while waiting for ARP'],
    explain: 'Unknown unicast flooding is normal switch behavior; once the destination is learned in the MAC table, unicast forwarding resumes.',
  },
  b6: {
    q: 'Which two ICMP message types does the ping command use?',
    options: [
      'Redirect and Unreachable',
      'Echo Request and Echo Reply',
      'Source Quench and Time Exceeded',
      'Router Advertisement and Solicitation',
    ],
    explain: 'ping measures reachability and round-trip time with Echo Request (type 8) and Echo Reply (type 0).',
  },
  b7: {
    q: 'Which of the following protocols is connection-oriented and provides acknowledgments and retransmission?',
    options: ['UDP', 'TCP', 'ICMP', 'ARP'],
    explain: 'TCP establishes a connection with a three-way handshake and uses sequence numbers, acknowledgments and retransmission to guarantee reliable delivery; UDP is connectionless and does not guarantee delivery.',
  },
  b8: {
    q: 'How long is an Ethernet MAC address?',
    options: ['32 bits', '48 bits', '64 bits', '128 bits'],
    explain: 'A MAC address is 48 bits (6 bytes): the first three bytes are the vendor OUI and the last three are assigned by the vendor.',
  },
  b9: {
    q: 'By default, each port on a switch is an independent what?',
    options: ['Broadcast domain', 'Collision domain', 'Routing domain', 'Autonomous system'],
    explain: 'Each switch port isolates a collision domain (with full duplex there are no collisions), while broadcast domains are separated by VLANs.',
  },
  b10: {
    q: 'When a host sends traffic to another subnet, what destination MAC goes into the Ethernet frame?',
    options: ['MAC of the destination host', 'MAC of the default gateway', 'Broadcast MAC', 'MAC of the DNS server'],
    explain: 'For off-subnet traffic the host sends the frame to the gateway: the destination IP stays the final target, but the destination MAC is the MAC of the gateway interface, and the gateway then forwards hop by hop.',
  },
  b11: {
    q: 'Which of the following addresses is an RFC1918 private address?',
    options: ['172.32.0.1', '192.169.1.1', '10.255.255.1', '11.0.0.1'],
    explain: 'The private ranges are 10.0.0.0/8, 172.16.0.0/12 (172.16 through 172.31) and 192.168.0.0/16; 10.255.255.1 falls inside 10/8.',
  },
  b12: {
    q: 'How many usable host addresses does a /30 subnet provide?',
    options: ['1', '2', '4', '6'],
    explain: 'A /30 has 2 host bits, giving 4 addresses in total; removing the network and broadcast addresses leaves 2 usable addresses, which is common for point-to-point links.',
  },
  b13: {
    q: 'What is the message order of the TCP three-way handshake?',
    options: ['SYN -> ACK -> SYN-ACK', 'SYN -> SYN-ACK -> ACK', 'ACK -> SYN -> SYN-ACK', 'SYN-ACK -> SYN -> ACK'],
    explain: 'The client sends SYN, the server answers with SYN-ACK, and the client replies with ACK to establish the connection; data transfer starts only after that.',
  },
  b14: {
    q: 'What is the main purpose of DNS?',
    options: ['Resolve domain names to IP addresses', 'Resolve IP addresses to MAC addresses', 'Assign IP addresses', 'Test link connectivity'],
    explain: 'DNS resolves human-readable domain names into IP addresses; ARP handles IP to MAC resolution and DHCP handles address assignment.',
  },
  b15: {
    q: 'When an IPv4 packet is larger than the outbound interface MTU and fragmentation is allowed, what does the router do?',
    options: ['Drops it', 'Fragments it and forwards', 'Rewrites the source MAC and increases the MTU', 'Converts it to UDP'],
    explain: 'IPv4 allows fragmentation in transit; if the DF bit is set the packet is dropped and ICMP unreachable (fragmentation needed) is returned, while IPv6 only fragments at the source.',
  },
  b16: {
    q: 'What is the advantage of full duplex over half duplex?',
    options: ['It can send and receive at the same time and does not need CSMA/CD', 'Longer transmission distance', 'Automatic IP assignment', 'It can cross VLANs'],
    explain: 'A full-duplex link uses separate channels for transmit and receive, so traffic flows both ways at once with no collisions, and CSMA/CD collision detection is no longer needed.',
  },
  b17: {
    q: 'traceroute shows the path hop by hop by relying on what?',
    options: ['ARP broadcasts', 'Decreasing TTL values that trigger time exceeded messages', 'DHCP leases', 'STP BPDUs'],
    explain: 'It sends probes with TTL=1, 2, 3 and so on; each router that decrements the TTL to 0 returns ICMP Time Exceeded, which reveals the address of that hop.',
  },
  b18: {
    q: 'A host wants to ping a device on the same subnet. What does it usually do before sending the ICMP message?',
    options: ['Query DNS', 'Send an ARP request to learn the target MAC', 'Open a TCP connection', 'Send a DHCP request to the gateway'],
    explain: 'Same-subnet traffic needs a destination MAC, so the host first sends an ARP broadcast asking for the MAC that matches the target IP, and only then encapsulates the ICMP frame.',
  },

  // ---------- VLAN ----------
  v1: {
    q: 'Two hosts in the same VLAN connect to two access ports on the same switch (VLAN 10). Do their frames carry an 802.1Q tag?',
    options: ['Tagged with VLAN 10', 'Untagged', 'Tagged with VLAN 1', 'Double tagged'],
    explain: 'Access ports distinguish VLANs only inside the switch and strip the tag when sending to a host; hosts see ordinary Ethernet frames.',
  },
  v2: {
    q: 'A trunk link between two switches allows VLAN 1 by default. To carry VLAN 10 traffic, what does a Huawei device need?',
    options: [
      'port default vlan 10',
      'port trunk allow-pass vlan 10',
      'port link-type access',
      'undo port trunk pvid',
    ],
    explain: 'Huawei and H3C trunks allow only VLAN 1 by default, so the VLAN must be permitted explicitly with allow-pass (permit on Comware); Cisco allows all VLANs by default.',
  },
  v3: {
    q: 'Inter-VLAN routing on a Layer 3 switch is usually implemented with what?',
    options: [
      'One physical port per VLAN',
      'SVI (VLANIF virtual interface)',
      'Router-on-a-stick subinterfaces',
      'An extra router with full mesh links',
    ],
    explain: 'The Layer 3 switch creates an SVI for each VLAN (for example Vlanif10) as the gateway of that VLAN, and packets are routed between VLANs internally.',
  },
  v4: {
    q: 'After traffic crosses VLANs (VLAN10 to VLAN20 routed through an SVI), how does the TTL of the reply compare with same-subnet traffic?',
    options: ['Unchanged', 'Decreased by 1', 'Decreased by 2', 'Reset to zero'],
    explain: 'Each Layer 3 forwarding step (routing between SVIs) decrements the TTL by 1, so a reply across VLANs arrives with a TTL one lower than on the same subnet.',
  },
  v5: {
    q: 'How does native VLAN (PVID/Native VLAN) traffic travel on a trunk link?',
    options: ['Tagged with the native VLAN', 'Untagged', 'Encrypted', 'Dropped'],
    explain: 'Frames in the native VLAN (called PVID on Huawei) are sent untagged on a trunk; mismatched PVIDs at the two ends cause VLAN leakage.',
  },
  v6: {
    q: 'How many bytes does an 802.1Q tag occupy in an Ethernet frame?',
    options: ['2 bytes', '4 bytes', '8 bytes', '12 bytes'],
    explain: 'The 802.1Q tag is 4 bytes: a 2-byte TPID (0x8100) plus a 2-byte TCI (priority, DEI, VLAN ID).',
  },
  v7: {
    q: 'What does the PVID of an access port do?',
    options: ['Tags every frame', 'Puts received untagged frames into that VLAN', 'Sets STP priority', 'Limits the number of MAC addresses'],
    explain: 'The PVID is the default VLAN of the port: an access port assigns untagged frames to the PVID and removes the tag on egress.',
  },
  v8: {
    q: 'A trunk port receives a frame tagged VLAN 30, but VLAN 30 is not in the allowed list. What does the switch do?',
    options: ['Forwards it as usual', 'Drops the frame', 'Changes it to VLAN 1 and forwards', 'Returns ICMP unreachable'],
    explain: 'Frames for VLANs that are not allowed are dropped immediately on trunk ingress, which is part of VLAN isolation.',
  },
  v9: {
    q: 'For security reasons, on which VLAN should the management address usually not be placed?',
    options: ['VLAN 1', 'A dedicated management VLAN', 'An unused service VLAN', 'A separate high-numbered VLAN'],
    explain: 'VLAN 1 is the default VLAN and almost every port may belong to it, so a dedicated management VLAN is the safer place for management addresses.',
  },
  v10: {
    q: 'For an SVI (Vlanif10) on a Layer 3 switch to be up, what is usually required?',
    options: ['Enable STP globally', 'At least one port in that VLAN is up', 'Configure a default route', 'Disable ARP'],
    explain: 'An SVI is a logical interface: with no member port up in the VLAN it normally stays down even when an IP address is configured.',
  },
  v11: {
    q: 'Why does the MAC address table keep entries per VLAN?',
    options: [
      'MAC addresses are globally unique, so there is no need',
      'The same MAC can appear in separate broadcast domains of different VLANs',
      'To save memory',
      'The VLAN tag is stored inside the MAC address',
    ],
    explain: 'Different VLANs are independent broadcast domains, so the same physical device (or a virtualized environment) can present the same MAC in different VLANs, and entries must be learned per (VLAN, MAC) pair.',
  },
  v12: {
    q: 'A host in VLAN10 needs to reach VLAN20. Where should its default gateway point?',
    options: ['The VLAN20 SVI address', 'The SVI address of its own VLAN', 'Any public address', 'The IP of the peer host'],
    explain: 'The host hands off-subnet traffic to the gateway of its own VLAN (Vlanif10), and the Layer 3 switch looks up a route to forward it into VLAN20.',
  },
  v13: {
    q: 'What is the configurable VLAN ID range?',
    options: ['0-4095', '1-4094', '1-1024', '1-65535'],
    explain: 'The VLAN ID is 12 bits, giving a usable range of 1-4094; 0 and 4095 are reserved and cannot be used as normal VLANs.',
  },
  v14: {
    q: 'Two switches are connected by a trunk with native VLAN 1 on one side and 99 on the other. What is the most likely result?',
    options: [
      'The link goes down immediately',
      'Untagged frames land in different VLANs, so VLAN leakage may occur',
      'The two ends negotiate a common VLAN automatically',
      'Only STP is affected, traffic is not',
    ],
    explain: 'Native VLAN frames carry no tag, so when the two ends disagree the same untagged frame is classified into different VLANs, creating a cross-VLAN leak risk.',
  },
  v15: {
    q: 'What is the H3C (Comware) command that permits VLAN 10 on a trunk port?',
    options: [
      'port trunk allow-pass vlan 10',
      'port trunk permit vlan 10',
      'switchport trunk allowed vlan 10',
      'port default vlan 10',
    ],
    explain: 'Comware uses port trunk permit vlan, Huawei VRP uses allow-pass, and Cisco uses switchport trunk allowed vlan.',
  },
  v16: {
    q: 'What is the complete Cisco IOS configuration that places a port in VLAN 10?',
    options: [
      'interface g0/1 -> switchport mode access -> switchport access vlan 10',
      'interface g0/1 -> port link-type access -> port default vlan 10',
      'interface g0/1 -> vlan 10',
      'vlan 10 -> interface g0/1',
    ],
    explain: 'IOS uses switchport mode access followed by switchport access vlan 10; the Huawei and H3C equivalents are port link-type access plus port default vlan.',
  },
  v17: {
    q: 'What is normally used to give a Layer 2 switch a management IP address?',
    options: ['Assign an IP to any access port', 'The SVI of the management VLAN', 'A loopback interface', 'An IP on the console port'],
    explain: 'A Layer 2 switch that needs a Layer 3 management address creates an SVI on the management VLAN, configures an IP on it, and then configures a default gateway for it.',
  },

  // ---------- STP ----------
  s1: {
    q: 'What determines the STP root bridge election?',
    options: [
      'The smallest bridge MAC wins',
      'The smallest bridge priority wins, with MAC as the tie-breaker',
      'The smallest IP address wins',
      'The switch with the most ports wins',
    ],
    explain: 'Bridge ID (priority plus MAC) is compared: priority first (default 32768, step 4096), then MAC if the priority ties, and the lower value wins.',
  },
  s2: {
    q: 'On a non-root switch, the port with the lowest cumulative path cost to the root bridge is called what?',
    options: ['Designated port (DP)', 'Root port (RP)', 'Alternate port', 'Edge port'],
    explain: 'Every non-root bridge has exactly one root port, the port with the lowest total cost toward the root; the remaining ports that would create a loop become alternate ports and are blocked.',
  },
  s3: {
    q: 'In 802.1D STP a port moves from Listening to Forwarding through Learning. How long does each stage take by default?',
    options: ['2 seconds', '15 seconds', '20 seconds', '50 seconds'],
    explain: 'The Forward Delay is 15 seconds by default, so Listening to Learning to Forwarding takes 15 seconds per stage, and together with the 20 second Max Age this gives the classic convergence time.',
  },
  s4: {
    q: 'What does a blocked STP port (alternate) do when it receives data frames?',
    options: ['Forwards them normally', 'Discards them but still receives BPDUs', 'Forwards and floods them', 'Returns a reject message'],
    explain: 'A blocked port does not forward data frames (to prevent loops) but keeps listening to BPDUs so it can take over quickly after a topology change; BPDU aging uses a Max Age of 20s.',
  },
  s5: {
    q: 'What is the most direct way to make a switch the root bridge?',
    options: [
      'Increase its bridge priority',
      'Lower its bridge priority (for example 4096)',
      'Disable STP on it',
      'Raise its port costs',
    ],
    explain: 'The lower the priority, the more likely a switch becomes root; Huawei and H3C use stp priority and Cisco uses spanning-tree vlan 1 priority (step 4096).',
  },
  s6: {
    q: 'What are the two parts of the STP bridge ID?',
    options: ['IP address plus port number', 'Bridge priority plus bridge MAC', 'VLAN ID plus bridge MAC', 'Path cost plus port ID'],
    explain: 'The bridge ID is a 2-byte priority plus a 6-byte MAC; comparison checks priority first and MAC second.',
  },
  s7: {
    q: 'In this simulator, what is the default STP path cost of a 1Gbps link?',
    options: ['2', '4', '19', '100'],
    explain: 'The simulator derives cost from link speed and counts 1Gbps as 4; exact values differ between standards (802.1D and 802.1t), but higher bandwidth always means lower cost.',
  },
  s8: {
    q: 'In 802.1D, what is the default Hello timer for configuration BPDUs?',
    options: ['1 second', '2 seconds', '15 seconds', '20 seconds'],
    explain: 'The root bridge sends a configuration BPDU every 2 seconds, and other switches relay it downstream while updating their own BPDU information.',
  },
  s9: {
    q: 'What is the default STP Max Age and what does it mean?',
    options: ['15 seconds, forwarding delay', '20 seconds, BPDU aging time', '2 seconds, Hello interval', '30 seconds, learning time'],
    explain: 'If a port receives no new BPDU within Max Age (20s), the neighbor is considered gone, the old information is discarded and the topology is recalculated.',
  },
  s10: {
    q: 'What is the role of a designated port?',
    options: [
      'Carry traffic toward the root bridge',
      'Send BPDUs onto its segment and forward traffic there',
      'Always block',
      'Only receive BPDUs',
    ],
    explain: 'Each segment elects one designated port that advertises BPDUs onto that segment and forwards traffic; the root port is the local direction toward the root.',
  },
  s11: {
    q: 'In what order do 802.1D port states progress?',
    options: [
      'Blocking -> Forwarding',
      'Blocking -> Listening -> Learning -> Forwarding',
      'Listening -> Blocking -> Learning -> Forwarding',
      'Learning -> Listening -> Forwarding',
    ],
    explain: 'A port chosen as root or designated starts in Listening, moves to Learning after the Forward Delay (15s), and reaches Forwarding 15s later.',
  },
  s12: {
    q: 'What port roles exist on the root bridge?',
    options: ['All of them are root ports', 'There is no root port; its link ports are designated ports', 'Half of them are blocked', 'All of them are alternate ports'],
    explain: 'The cost from the root bridge to itself is 0, so no root port is needed; the root bridge is designated on every segment (all ports forward when there is no loop).',
  },
  s13: {
    q: 'After STP detects a change of root port, what does the switch do to speed up convergence?',
    options: ['Flushes the MAC address table and signals a topology change', 'Restarts all ports', 'Shuts down VLANs', 'Resets ARP'],
    explain: 'After a topology change (TCN) the switch flushes its MAC table so traffic stops following the old path; this simulator approximates it by flushing MAC entries network-wide.',
  },
  s14: {
    q: 'When two ports have the same path cost to the root, what does STP compare next?',
    options: ['Port speed', 'Upstream bridge ID / port ID', 'IP address', 'MAC address length'],
    explain: 'With equal path cost STP compares the upstream bridge ID (the sender of the BPDU), then the upstream port ID, and the lower value wins.',
  },
  s15: {
    q: 'What is the core advantage of RSTP (802.1w) over 802.1D?',
    options: [
      'Faster convergence (alternate ports and propose-agree)',
      'No root bridge is needed',
      'It can forward across VLANs',
      'No BPDUs at all',
    ],
    explain: 'RSTP adds the alternate port role and the propose-agree handshake, so a link failure can be handled in sub-second time; 802.1D often needs 30-50 seconds.',
  },
  s16: {
    q: 'What is most likely to happen if STP is disabled in a network that has a loop?',
    options: ['The loop disappears automatically', 'A broadcast storm floods the switches and links', 'VLANs are isolated automatically', 'Traffic switches to Layer 3 routing'],
    explain: 'Without STP to block redundant links, broadcast frames are replicated endlessly around the loop, creating a broadcast storm that exhausts bandwidth and CPU.',
  },
  s17: {
    q: 'Two switches are joined by two links bundled into a link aggregation group. What does STP see?',
    options: ['Two independent links, one of which must be blocked', 'One logical link, so both members can forward at once', 'Three links', 'No link'],
    explain: 'An aggregation group appears to STP as a single logical port, so members are not blocked by mistake; bandwidth adds up and traffic can switch over on failure.',
  },
};
