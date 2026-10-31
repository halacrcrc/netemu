// English translations, part 2: DHCP, ACL and NAT questions.
// Derived from the original questions in questions.ts; answer positions are unchanged.

/**
 * Mirrors the QuestionEn shape used in questions-en-1.ts.
 * Defined locally because that module is not always present.
 */
export interface QuestionEn {
  /** Question text */
  q: string;
  /** Four options, in the original order */
  options: string[];
  /** Explanation */
  explain: string;
}

export const QUESTIONS_EN_2: Record<string, QuestionEn> = {
  // ---------- DHCP ----------
  d1: {
    q: 'What is the four-step DHCP message exchange order?',
    options: [
      'Offer → Discover → Ack → Request',
      'Discover → Offer → Request → Ack',
      'Request → Discover → Ack → Offer',
      'Discover → Request → Offer → Ack',
    ],
    explain:
      'DORA: the client broadcasts a Discover, the server answers with an Offer, the client broadcasts a Request to confirm, and the server sends an Ack that completes the lease.',
  },
  d2: {
    q: 'What are the source and destination IP addresses of a DHCP Discover message?',
    options: [
      '0.0.0.0 → 255.255.255.255',
      '127.0.0.1 → 127.0.0.1',
      'client MAC → server MAC',
      '169.254.x.x → 224.0.0.1',
    ],
    explain:
      'The client has no address yet, so the source is 0.0.0.0 and the message goes to the limited broadcast 255.255.255.255 — which is why DHCP broadcasts need a relay to cross a Layer 3 network.',
  },
  d3: {
    q: 'How does a DHCP server usually decide which address pool to allocate an address from?',
    options: [
      'The hostname of the client',
      'The subnet of the interface on which the request was received',
      'The order in which the requests arrive',
      'The VLAN specified by the client',
    ],
    explain:
      'The router answers from the pool on the same subnet as the interface that received the Discover — this is also how pool matching works in this simulator.',
  },
  d4: {
    q: 'Besides an IP address and subnet mask, what does DHCP usually deliver as well?',
    options: [
      'Default gateway and DNS servers',
      'MAC address and port number',
      'VLAN ID and trunk configuration',
      'STP priority',
    ],
    explain:
      'Common options: option 3 default gateway, option 6 DNS, option 1 subnet mask — without a gateway a client cannot communicate beyond its own subnet.',
  },
  d5: {
    q: 'Two PCs obtain addresses from the same pool one after another. What address does the second PC get?',
    options: [
      'The same one as the first PC',
      'The next free address in the pool',
      'Always the broadcast address',
      'A random address',
    ],
    explain:
      'The server keeps a lease table: addresses already leased are not handed out again, so the second PC gets the next free address (this simulator allocates sequentially from the start of the pool).',
  },
  d6: {
    q: 'Which UDP ports do the DHCP server and the DHCP client use?',
    options: [
      'Server 67, client 68',
      'Server 68, client 67',
      'Both use 69',
      'Server 80, client 443',
    ],
    explain: 'The server listens on UDP 67 and the client uses UDP 68; both are well-known ports.',
  },
  d7: {
    q: 'What problem does a DHCP relay solve?',
    options: [
      'It converts broadcast Discovers into unicasts toward a server on another subnet',
      'It encrypts DHCP messages',
      'It speeds up address allocation',
      'It prevents address conflicts',
    ],
    explain:
      'DHCP broadcasts cannot cross a Layer 3 boundary; a relay (ip helper-address / dhcp relay) forwards the request to a remote server and passes the reply back.',
  },
  d8: {
    q: 'When half of the lease has elapsed, how does a client usually send its renewal?',
    options: [
      'A broadcast Discover',
      'A unicast Request to the original server',
      'A complete DORA sequence again',
      'An ARP request',
    ],
    explain:
      'The client first unicasts a Request to the original server to renew; only if that fails does it broadcast a new Discover as the lease nears expiry.',
  },
  d9: {
    q: 'Reserving a fixed IP for a printer based on its MAC address is called?',
    options: [
      'Dynamic allocation',
      'Static binding / address reservation',
      'Address relay',
      'NAT mapping',
    ],
    explain:
      'Binding a fixed address to a client MAC inside the pool (static binding/reservation) keeps a device address from changing under dynamic DHCP allocation.',
  },
  d10: {
    q: 'What is the purpose of configuring "excluded addresses" in an address pool?',
    options: [
      'To keep addresses for servers, gateways and similar devices out of dynamic allocation',
      'To speed up allocation',
      'To limit the number of clients',
      'To translate addresses into public addresses',
    ],
    explain:
      'Excluded ranges are normally left for manually configured devices (gateways, servers, printers) so that DHCP does not hand the same address out again.',
  },
  d11: {
    q: 'After receiving an Offer, how does the client usually send its Request?',
    options: [
      'Unicast to the server that sent the Offer',
      'Broadcast, telling all servers which one it has chosen',
      'To the DNS server',
      'Only as an ARP',
    ],
    explain:
      'The Request is broadcast, which confirms the selected server and lets any other server that sent an Offer release its reservation.',
  },
  d12: {
    q: 'A Windows host that cannot obtain a DHCP address often shows 169.254.x.x. That address is?',
    options: [
      'A public address',
      'An APIPA automatic private address',
      'A multicast address',
      'A loopback address',
    ],
    explain:
      'APIPA (169.254.0.0/16) is a local address a client generates automatically when no DHCP is available; it only allows communication within the same subnet.',
  },
  d13: {
    q: 'When the DHCP server and the client are on different subnets, you must?',
    options: [
      'Change the server address',
      'Forward through a DHCP relay/helper',
      'Configure a gateway manually on the client',
      'Disable ARP',
    ],
    explain:
      'Client broadcasts cannot cross a router, so a DHCP relay pointing at the server must be configured on the gateway or Layer 3 device.',
  },
  d14: {
    q: 'With which message does the client finally confirm that it accepts the address?',
    options: ['Discover', 'Offer', 'Request', 'Ack'],
    explain:
      'The client sends a Request to confirm its choice, and the address only becomes usable after the server replies with an Ack; the Ack also carries options such as the mask, gateway and DNS.',
  },
  d15: {
    q: 'What is the main security purpose of DHCP Snooping?',
    options: [
      'Block unauthorized DHCP servers and build a binding table on trusted ports',
      'Encrypt DHCP messages',
      'Speed up lease reclamation',
      'Replace NAT',
    ],
    explain:
      'Snooping separates trusted from untrusted ports and drops DHCP replies arriving on untrusted ports, preventing a privately deployed server from hijacking the gateway; it also records a binding table.',
  },
  d16: {
    q: 'When two DHCP servers exist on the same subnet, the client usually?',
    options: [
      'Accepts all Offers',
      'Uses the first Offer received and sends a Request',
      'Picks a server at random',
      'Rejects all Offers',
    ],
    explain:
      'The client normally takes the first Offer and broadcasts a Request to confirm, and the remaining servers reclaim their reservations.',
  },
  d17: {
    q: 'Which of the following does DHCP assign?',
    options: ['MAC address', 'IP address and network parameters', 'VLAN ID', 'STP priority'],
    explain:
      'DHCP assigns the IP address, mask, gateway, DNS and other network parameters; MAC addresses are burned in by the NIC vendor and are not assigned by DHCP.',
  },

  // ---------- ACL ----------
  a1: {
    q: 'ACL matching is "first match wins". If a packet matches no rule at all, what is the result?',
    options: [
      'Permitted by default',
      'Denied by default (implicit deny)',
      'Matching continues in the next ACL',
      'An error message is returned',
    ],
    explain:
      'An invisible implicit deny all sits at the end of every ACL — when writing an ACL, permit what you need and let everything else fall through to the deny.',
  },
  a2: {
    q: 'In VRP, ACL numbers 3000–3999 belong to which category?',
    options: ['Basic ACL', 'Advanced ACL', 'Layer 2 ACL', 'User ACL'],
    explain:
      'Huawei/H3C: 2000–2999 basic (source only), 3000–3999 advanced (source + destination + protocol); the Cisco equivalents are 1–99 standard and 100–199 extended.',
  },
  a3: {
    q: 'The wildcard mask 0.0.0.255 means that matching?',
    options: [
      'Matches a single host only',
      'The last byte can be anything',
      'The first three bytes can be anything',
      'All 32 bits must match exactly',
    ],
    explain:
      'In a wildcard, 0 means the bit must match and 1 means it is ignored: 0.0.0.255 is the first 24 bits exact with the last 8 bits arbitrary, i.e. a /24 subnet.',
  },
  a4: {
    q: 'When an ACL is applied inbound on an interface, when is the decision made?',
    options: [
      'After the routing table lookup',
      'Before the routing table lookup',
      'Only when the frame leaves the interface',
      'Only on returning packets',
    ],
    explain:
      'An inbound ACL is evaluated as soon as the packet enters the interface (before routing); an outbound ACL is evaluated after routing and before the packet leaves the interface.',
  },
  a5: {
    q: 'Which fields of a packet can a standard (basic) ACL match?',
    options: ['Source IP address', 'Source and destination IP', 'TCP port', 'MAC address'],
    explain:
      'A standard ACL looks only at the source IP (Cisco 1–99, Huawei/H3C 2000–2999); matching the destination, protocol or port requires an extended/advanced ACL.',
  },
  a6: {
    q: 'To deny access to TCP port 80 on a server, you should use?',
    options: [
      'A standard ACL',
      'An extended/advanced ACL',
      'A MAC ACL',
      'Port security only',
    ],
    explain:
      'Ports are transport-layer information, so only extended (Cisco 100–199) / advanced (3000–3999) ACLs can match protocol and port.',
  },
  a7: {
    q: 'What is the number range of Cisco IOS extended ACLs?',
    options: ['1–99', '100–199', '200–299', '1000–1999'],
    explain:
      'IOS: 1–99 standard, 100–199 extended; the Huawei/H3C equivalents are 2000–2999 basic and 3000–3999 advanced.',
  },
  a8: {
    q: 'When one ACL is referenced by several interfaces, modifying its rules will?',
    options: [
      'Affect only the first interface',
      'Immediately affect every interface that references it',
      'Require a device reboot',
      'Automatically create copies',
    ],
    explain:
      'An ACL is a global object that interfaces only reference; after a rule change, every interface where it is applied immediately evaluates against the new rules.',
  },
  a9: {
    q: 'The wildcard mask 0.0.0.0 means?',
    options: [
      'Match any address',
      'Match that host address exactly',
      'Match the whole subnet',
      'Ignore that address',
    ],
    explain:
      'A wildcard bit of 0 means the bit must match, so all zeros is an exact match for a single host; many platforms let you abbreviate this to the host form.',
  },
  a10: {
    q: 'To "permit only 192.168.1.10 and deny everything else", the rules should be ordered?',
    options: [
      'deny any first, then permit the host',
      'permit host 192.168.1.10 first, then deny any',
      'A single permit is enough',
      'Order does not matter',
    ],
    explain:
      'Because the first match wins, the specific host must be permitted first and everything else denied afterwards; reversing the order would deny all traffic up front.',
  },
  a11: {
    q: 'What is the main benefit of applying an ACL inbound rather than outbound?',
    options: [
      'It can match more fields',
      'Traffic is dropped as soon as it arrives, saving later forwarding resources',
      'It encrypts automatically',
      'It bypasses the routing table',
    ],
    explain:
      'Filtering early on ingress avoids unnecessary route lookups and forwarding and relieves pressure on downstream links; which direction to use depends on the design.',
  },
  a12: {
    q: 'Which command applies advanced ACL 3000 inbound on an interface in Huawei VRP?',
    options: [
      'traffic-filter inbound acl 3000',
      'ip access-group 3000 in',
      'packet-filter 3000 inbound',
      'acl 3000 inbound',
    ],
    explain:
      'VRP uses traffic-filter inbound acl, Comware uses packet-filter, and Cisco uses ip access-group.',
  },
  a13: {
    q: 'When viewing ACL rules, what does the matched counter represent?',
    options: [
      'The rule number',
      'The number of packets that have hit the rule',
      'The ACL priority',
      'The number of remaining available entries',
    ],
    explain:
      'The hit counter helps troubleshooting: it confirms whether a rule is actually matching traffic (this simulator shows it with display acl all).',
  },
  a14: {
    q: 'In a Huawei advanced ACL, the fragment matching "destination TCP port 80" is usually?',
    options: [
      'destination-port eq 80',
      'source-port eq 80',
      'tcp 80 any',
      'port 80 permit',
    ],
    explain:
      'After the protocol tcp is specified, an advanced ACL can carry conditions such as destination-port/source-port, for example tcp destination-port eq 80.',
  },
  a15: {
    q: 'Which command applies an ACL on an interface in H3C (Comware)?',
    options: [
      'packet-filter 3000 inbound',
      'traffic-filter inbound acl 3000',
      'ip access-group 3000 in',
      'acl 3000 apply',
    ],
    explain:
      'Comware uses packet-filter <acl> <direction>; VRP uses traffic-filter and IOS uses ip access-group.',
  },
  a16: {
    q: 'What is the key difference between a wildcard mask and a subnet mask?',
    options: [
      'In a wildcard, 1 means ignore and 0 means match — the reverse of a subnet mask',
      'They are exactly the same',
      'Wildcards can only be used with IPv6',
      'A subnet mask can only be 0.0.0.0',
    ],
    explain:
      'In a subnet mask contiguous 1s mark the network bits; in a wildcard 0 means must match and 1 means anything, so for example 0.0.0.255 corresponds to /24.',
  },
  a17: {
    q: 'In NAT, an ACL specifies "which source addresses are translated". What is its role?',
    options: [
      'Only matching traffic is translated, everything else is left unchanged',
      'Matching traffic is encrypted',
      'Matching traffic is given a DNS server',
      'Matching traffic is blocked',
    ],
    explain:
      'NAT is often combined with an ACL: only sources the ACL permits (or matches) are translated, which allows different translation policies for internal/external or special addresses.',
  },

  // ---------- NAT ----------
  n1: {
    q: 'With Easy-IP (PAT), when several internal hosts share one internet connection, what does the router rewrite?',
    options: [
      'The destination IP into a public address',
      'The source IP into the egress public IP, with replies distinguished per session',
      'The TTL into 128',
      'The MAC address into the gateway MAC',
    ],
    explain:
      'PAT translates the internal source address plus port into the egress public address plus port and records the mapping, restoring replies from that table — this is exactly how several hosts share one public address.',
  },
  n2: {
    q: 'What is the purpose of the inside/outside interface markings on a NAT router?',
    options: [
      'To divide VLANs',
      'To determine in which direction addresses are translated',
      'To specify the DNS server',
      'To enable the DHCP service',
    ],
    explain:
      'Packets going from inside to outside get source translation (outbound), and returning packets get their destination restored on the outside interface (inbound).',
  },
  n3: {
    q: 'What is the difference between static NAT and Easy-IP?',
    options: [
      'Static NAT is a fixed one-to-one mapping; Easy-IP dynamically reuses the egress address',
      'Static NAT only supports TCP',
      'Easy-IP does not need routing',
      'There is no difference',
    ],
    explain:
      'Static NAT lets the outside reach that internal server on its own (fixed 1:1); with Easy-IP sessions are created from the inside, so the outside cannot initiate a connection.',
  },
  n4: {
    q: 'An internal PC pings an external server through NAT. What does the translation table record so replies can be restored?',
    options: [
      'A mapping between the internal source IP and the ICMP session ID',
      'The MAC address of the destination server',
      'The original TTL value',
      'The public MAC of the server',
    ],
    explain:
      'The (internal source IP, id) pair of the ICMP echo is recorded; when a reply arrives at the egress IP with a matching id, it is restored to the internal IP and original id and sent back to the initiator.',
  },
  n5: {
    q: 'Which of the following does **not** require NAT?',
    options: [
      'Several internal PCs accessing the internet',
      'An internal server offering web services to the public internet',
      'Two PCs in the same VLAN communicating with each other',
      'An office network and a partner network with overlapping addresses communicating',
    ],
    explain:
      'Communication inside the same VLAN is pure Layer 2 switching with no Layer 3 address translation; the other scenarios are typical uses of NAT.',
  },
  n6: {
    q: 'What is the core purpose of NAT?',
    options: [
      'Translate private addresses into addresses that can be routed on the public internet',
      'Translate domain names into IP addresses',
      'Translate MAC addresses into IP addresses',
      'Encrypt internal traffic',
    ],
    explain:
      'Private addresses cannot be routed on the internet; NAT rewrites addresses on the border device so that internal hosts can share a public address to reach the outside.',
  },
  n7: {
    q: 'How does PAT distinguish the same connection from several internal hosts?',
    options: [
      'By MAC address',
      'By the mapping of source port / ICMP session ID',
      'By TTL',
      'By VLAN tag',
    ],
    explain:
      'PAT maps (internal IP, port) to (public IP, port) and restores returning packets precisely to the initiator by port/session ID.',
  },
  n8: {
    q: 'In NAT terminology, what does inside local refer to?',
    options: [
      'The address an internal host appears as on the public internet',
      'The private address an internal host actually uses',
      'The address of the external server',
      'The gateway MAC',
    ],
    explain:
      'inside local = the private address of the internal host itself; inside global = the public address it presents after NAT.',
  },
  n9: {
    q: 'To let internet users reach an internal web server on their own initiative, you normally configure?',
    options: [
      'Easy-IP/PAT',
      'Static NAT or port mapping',
      'A DHCP static binding',
      'VLAN isolation',
    ],
    explain:
      'Easy-IP can only be initiated from the inside; outside-initiated access requires a static one-to-one mapping or port mapping (destination NAT).',
  },
  n10: {
    q: 'What typical problem can NAT cause for IPsec?',
    options: [
      'Rewritten addresses/ports break integrity checks and endpoint identification',
      'IPsec can no longer encrypt',
      'It causes STP loops',
      'It makes DHCP fail',
    ],
    explain:
      'Rewriting the IP header breaks integrity checks such as AH and makes IKE endpoints inconsistent; NAT-T or a traversal solution is usually needed.',
  },
  n11: {
    q: 'Entries in a NAT translation table usually?',
    options: [
      'Are kept forever',
      'Age out and are removed on session timeout',
      'Only keep ICMP',
      'Are rebuilt for every packet',
    ],
    explain:
      'PAT entries age out with the session (TCP state/timeout) and the port is reclaimed afterwards; static NAT mappings persist for a long time.',
  },
  n12: {
    q: 'Mapping public port 8080 to port 80 on an internal server is?',
    options: ['Source NAT', 'Destination NAT / port mapping', 'DHCP relay', 'ACL filtering'],
    explain:
      'Rewriting the destination address/port so that external users reach an internal service is called destination NAT (port mapping / server publishing).',
  },
  n13: {
    q: 'In the outbound direction of a router, which is evaluated first — ACL or NAT?',
    options: [
      'The ACL, before NAT rewrites the packet',
      'NAT, before the ACL',
      'Both at the same time',
      'It depends on the interface',
    ],
    explain:
      'Consistent with real devices: outbound, the ACL matches on the original addresses first and NAT rewrites afterwards; inbound, the destination address is restored first and then processed.',
  },
  n14: {
    q: 'What is an important prerequisite for NAT to work?',
    options: [
      'Traffic in both directions must pass through the same NAT device',
      'Both ends must be in the same VLAN',
      'Static addresses must be used',
      'Routing must be disabled',
    ],
    explain:
      'The translation table lives on the NAT device, so the return path must traverse the same device to be restored; multiple exit points or asymmetric paths leave replies unmatched.',
  },
  n15: {
    q: 'Which translation mechanism lets an IPv6-only network reach IPv4 services?',
    options: ['NAT44', 'NAT64', 'PAT', 'STP'],
    explain:
      'NAT64 translates addresses between IPv6 and IPv4 (usually together with DNS64); NAT44 only handles IPv4 to IPv4.',
  },
  n16: {
    q: 'Which Huawei VRP command enables Easy-IP on an interface?',
    options: ['nat outbound', 'ip nat outside', 'nat static', 'nat inside'],
    explain:
      'VRP runs nat outbound on the egress interface (often combined with an ACL); Cisco uses ip nat outside plus ip nat inside source … overload.',
  },
  n17: {
    q: 'When Easy-IP translates addresses, which address is used as the translated source?',
    options: [
      'Any public address',
      'The IP address of the egress interface itself',
      'The DNS server address',
      'The internal gateway address',
    ],
    explain:
      'Easy-IP needs no address pool; it uses the IP of the egress (outside) interface as the translated source and multiplexes ports to separate sessions.',
  },
};
