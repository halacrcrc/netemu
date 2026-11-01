// English translations, part 5: intra-VLAN isolation, OSPFv3, SRv6.
// Source of truth: questions.ts. Option order and correct answer positions are preserved.

export interface QuestionEn { q: string; options: string[]; explain: string }

export const QUESTIONS_EN_5: Record<string, QuestionEn> = {
  // ---------- Intra-VLAN isolation ----------
  iso1: {
    q: 'What is characteristic of a MUX VLAN principal port?',
    options: ['It can only reach the gateway', 'It can communicate with all ports', 'It can only reach its group', 'It is fully isolated'],
    explain: 'The principal port (Cisco promiscuous) usually faces the gateway or servers and can communicate with every port.',
  },
  iso2: {
    q: 'Layer-2 communication between two separate ports is:',
    options: ['Allowed', 'Isolated', 'Broadcast only', 'MAC dependent'],
    explain: 'Separate (isolated) ports are isolated from each other and can only reach the principal.',
  },
  iso3: {
    q: 'MUX VLAN group ports:',
    options: ['Are all isolated', 'Communicate within the group but not across groups', 'All communicate', 'Only broadcast'],
    explain: 'Group (community) ports communicate inside their group, are isolated across groups, and can reach the principal.',
  },
  iso4: {
    q: 'In Cisco Private VLAN, which port can reach every port?',
    options: ['Isolated port', 'Community port', 'Promiscuous port', 'Trunk port'],
    explain: 'A promiscuous port can reach all ports in the primary VLAN; isolated and community ports are restricted.',
  },
  iso5: {
    q: 'In port isolation, ports in the same isolation group:',
    options: ['Can communicate', 'Cannot communicate at Layer 2', 'Can only communicate through the gateway', 'Are automatically aggregated'],
    explain: 'Ports in the same isolation group are isolated at Layer 2; different groups can normally communicate.',
  },

  // ---------- OSPFv3 ----------
  ov1: {
    q: 'What is the OSPFv3 Router ID?',
    options: ['A 128-bit IPv6 address', 'A 32-bit IPv4-format value', 'A MAC address', 'A 16-bit integer'],
    explain: 'OSPFv3 still uses a 32-bit Router ID, usually configured manually or taken from an IPv4 address.',
  },
  ov2: {
    q: 'OSPFv3 neighbors are usually formed over:',
    options: ['Global unicast', 'Link-local addresses', 'Multicast MAC', 'Loopback addresses'],
    explain: 'OSPFv3 adjacencies typically form over fe80::/10 link-local addresses, and next hops are link-local.',
  },
  ov3: {
    q: 'Which OSPFv3 LSA describes link-local addresses and prefixes?',
    options: ['Router-LSA', 'Network-LSA', 'Link-LSA', 'External-LSA'],
    explain: 'Link-LSAs carry the interface link-local address and IPv6 prefixes; SPF still uses Router-LSAs.',
  },
  ov4: {
    q: 'What about authentication in the OSPFv3 header?',
    options: ['Same as OSPFv2', 'Removed; IPv6 IPsec is used', 'MD5 only', 'Uses TLS'],
    explain: 'OSPFv3 removed authentication from the header; authentication relies on IPv6 IPsec.',
  },
  ov5: {
    q: 'In an IPv6 routing table, OSPFv3 routes are usually marked:',
    options: ['C', 'S', 'O', 'B'],
    explain: 'OSPFv3 routes are marked O; C is connected, S static, B BGP.',
  },

  // ---------- SRv6 ----------
  sr1: {
    q: 'An SRv6 SID is fundamentally:',
    options: ['An MPLS label', 'An IPv6 address', 'A MAC address', 'A VLAN ID'],
    explain: 'SRv6 uses IPv6 addresses (Locator + Function) as segment identifiers and does not rely on MPLS labels.',
  },
  sr2: {
    q: 'What is the role of an SRv6 Locator?',
    options: ['Assign MAC addresses', 'Identify a node/location in the SRv6 domain', 'Encrypt packets', 'Elect a DR'],
    explain: 'A Locator is an IPv6 prefix identifying a node/location, and SIDs live inside it.',
  },
  sr3: {
    q: 'What is characteristic of SRv6 BE?',
    options: ['Encapsulates toward a single remote SID', 'Requires a multi-segment list', 'Does not use IPv6', 'Layer-2 forwarding only'],
    explain: 'BE encapsulates with a single remote SID as the outer destination; the underlay forwards by IPv6 routing and the egress decapsulates.',
  },
  sr4: {
    q: 'What does SRv6 Policy use to express an explicit path?',
    options: ['A single SID', 'A Segment List (SID sequence)', 'A VLAN list', 'An ACL'],
    explain: 'Policy uses a segment list to specify a multi-segment path for traffic engineering and slicing.',
  },
  sr5: {
    q: 'What does TI-LFA do?',
    options: ['Provides topology-independent loop-free backup with fast failover', 'Allocates SIDs', 'Encrypts tunnels', 'Elects a root bridge'],
    explain: 'TI-LFA precomputes loop-free backup paths and switches in sub-second time after a failure.',
  },
};
