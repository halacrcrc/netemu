---
title: "Network Services and Management: SSH, NTP, SNMP, Syslog"
order: 26
level: IA
topic: services
lab: mgmt-ssh
---

A network must not only be reachable; it must be manageable, auditable, and time-synchronized. These services live mostly on the management plane — misconfiguring them will not break traffic immediately, but it will leave you blind during an outage.

## Remote management: Telnet and SSH

- **Telnet**: cleartext; passwords can be read straight from a capture, so it belongs only in a lab;
- **SSH**: encrypted transport, supports key authentication, the minimum for production;
- **AAA**: authentication (who), authorization (what they may do), accounting (what they did).

```
Huawei/H3C:
aaa
 local-user admin password cipher Admin@123
user-interface vty 0 4
 authentication-mode aaa
 protocol inbound ssh
Cisco:
username admin privilege 15 secret Admin@123
line vty 0 4
 login local
 transport input ssh
```

## Time, logs, and monitoring

- **NTP**: synchronize clocks so log timestamps line up; lower stratum is closer to the authoritative clock;
- **Syslog**: centralize logs; filter by severity (0 emergency through 7 debug);
- **SNMP**: read interface counters, CPU, and temperature; v3 adds authentication and encryption, while v1/v2c communities are effectively cleartext passwords;
- **FTP/TFTP/HTTP**: back up configurations and upgrade images; TFTP is simple and unauthenticated, suitable only for controlled networks.

## A practical management baseline

1. Dedicated management VLAN, restricted by ACL;
2. SSH only, Telnet disabled;
3. Strong local passwords plus centralized AAA;
4. NTP, centralized Syslog, SNMPv3;
5. Regular configuration backups and change records.

## Hands-on

Open Lab 6: configure local user admin and allow SSH only on the VTY lines. PC1 logs in with `ssh 192.168.1.1`, while `telnet 192.168.1.1` is rejected. Then consider: if `transport input ssh` is set but no local user exists, will login succeed?
