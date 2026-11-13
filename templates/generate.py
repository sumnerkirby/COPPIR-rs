#!/usr/bin/env python3
"""Write the built-in exercise templates as .coppir files.

    python3 templates/generate.py

The output is committed and compiled into the binary (src/templates.rs). A
Rust test checks that every template adopts cleanly and that every inject in
it refers to things that exist, so a mistake here fails `cargo test`.

Everything in these scenarios is fictional.
"""

import json
import pathlib

OUT = pathlib.Path(__file__).parent
T0 = "2026-01-01T00:00:00+00:00"

# ───────────────────────── shared vocabulary ─────────────────────────


def value(id, label, color, glyph, score):
    return {"id": id, "label": label, "color": color, "glyph": glyph, "score": score}


SECURITY = {
    "id": "security",
    "name": "Security",
    "display": "fill",
    "default": "clean",
    "values": [
        value("clean", "Clean", "#33cc33", "✓", 100),
        value("monitored", "Monitored", "#3399ff", "◦", 70),
        value("contained", "Contained", "#ff6600", "▪", 40),
        value("under_investigation", "Under Investigation", "#ffaa00", "?", 15),
        value("compromised", "Compromised", "#ff3333", "✕", 0),
    ],
}

OPERATIONAL = {
    "id": "operational",
    "name": "Operational",
    "display": "ring",
    "default": "healthy",
    "values": [
        value("healthy", "Healthy", "#00ff41", "solid", 100),
        value("degraded", "Degraded", "#ffd700", "dashed", 60),
        value("critical", "Critical", "#ff4500", "double", 25),
        value("offline", "Offline", "#888888", "dotted", 0),
    ],
}

PHASE_COLORS = ["#3399ff", "#ffaa00", "#ff6633", "#33cc99", "#aa88ff"]
TEAM_COLORS = ["#3399ff", "#ffaa00", "#aa88ff", "#00ccaa", "#ff6688"]

DEPENDS_IMPACT = {
    "id": "dependency-outage",
    "kind": "depends_on",
    "set": "operational",
    "when": ["offline", "critical"],
    "then_set": "operational",
    "then_value": "degraded",
}


class Builder:
    """Accumulates one exercise. Ids are readable slugs, kept stable across runs."""

    def __init__(self, name, description, banner):
        self.ex = {
            "id": "template",
            "name": name,
            "description": description,
            "taxonomy": {"asset_types": [], "status_sets": [SECURITY, OPERATIONAL], "sectors": [], "fields": []},
            "teams": [],
            "assets": {},
            "links": {},
            "injects": {},
            "thresholds": [],
            "log": [],
            "annotations": {},
            "metrics": [],
            "clock": {"elapsed_ms": 0, "running_since": None},
            "phases": [],
            "phase_log": [],
            "polls": {},
            "impact_rules": [DEPENDS_IMPACT],
            "branding": {
                "theme": "terminal",
                "accent": None,
                "org": "",
                "logo": None,
                "banner": {"text": banner, "color": "#b8860b", "position": "both"},
            },
            "created_at": T0,
            "updated_at": T0,
        }

    def types(self, *types):
        for id, name, icon, color in types:
            self.ex["taxonomy"]["asset_types"].append({"id": id, "name": name, "icon": icon, "color": color})

    def sector(self, id, name, color, types):
        self.ex["taxonomy"]["sectors"].append(
            {"id": id, "name": name, "color": color, "asset_types": types, "status_set": "security"}
        )

    def field(self, id, label, kind, player_visible=False, options=None):
        k = {"type": kind}
        if kind == "select":
            k["options"] = options
        self.ex["taxonomy"]["fields"].append({"id": id, "label": label, "kind": k, "player_visible": player_visible})

    def team(self, id, name):
        self.ex["teams"].append({"id": id, "name": name, "color": TEAM_COLORS[len(self.ex["teams"]) % 5]})

    def phases(self, *phases):
        for i, (id, name, minutes) in enumerate(phases):
            self.ex["phases"].append({"id": id, "name": name, "color": PHASE_COLORS[i % 5], "target_minutes": minutes})

    def asset(self, id, name, type_id, *, at=None, geo=None, security="clean", operational="healthy",
              notes="", fields=None, visible=None, tags=()):
        statuses = {"security": security, "operational": operational}
        self.ex["assets"][id] = {
            "id": id,
            "name": name,
            "type_id": type_id,
            "geo": {"lat": geo[0], "lon": geo[1]} if geo else None,
            "graph": {"x": at[0], "y": at[1]} if at else None,
            "statuses": statuses,
            "history": [{"set": s, "value": v, "at": T0} for s, v in statuses.items()],
            "fields": fields or {},
            "tags": list(tags),
            "notes": notes,
            "color": None,
            "visibility": visible or {"mode": "all"},
            "created_at": T0,
        }

    def link(self, provider, dependant, label="", kind="depends_on"):
        id = f"{provider}--{dependant}"
        self.ex["links"][id] = {"id": id, "from": provider, "to": dependant, "kind": kind, "label": label}

    def poll(self, id, question, options, *, audience=None, minutes=None, text=True):
        self.ex["polls"][id] = {
            "id": id,
            "question": question,
            "options": [{"id": f"{id}-{i}", "label": label} for i, label in enumerate(options)],
            "allow_text": text,
            "audience": audience or {"mode": "all"},
            "state": "draft",
            "duration_minutes": minutes,
            "opened_at": None,
            "closes_at": None,
            "closed_at": None,
            "responses": [],
            "results": {},
        }

    def inject(self, id, title, body, *, severity="warning", trigger=None, effects=(), audience=None, notes=""):
        self.ex["injects"][id] = {
            "id": id,
            "title": title,
            "body": body,
            "severity": severity,
            "audience": audience or {"mode": "all"},
            "effects": list(effects),
            "notes": notes,
            "created_at": T0,
            "fired_at": None,
            "trigger": trigger or {"type": "manual"},
            "armed": True,
            "expected": [],
        }

    def expect(self, inject, *actions):
        """What the players should do after an inject: (team or None, text) pairs."""
        lst = self.ex["injects"][inject]["expected"]
        for team, text in actions:
            lst.append({"id": f"{inject}-x{len(lst) + 1}", "text": text, "team": team, "verdict": None, "note": ""})

    def threshold(self, id, name, sector, below, severity="critical"):
        self.ex["thresholds"].append({"id": id, "name": name, "sector": sector, "below_pct": below, "severity": severity})

    def metric(self, id, name, value):
        self.ex["metrics"].append({"id": id, "name": name, "value": value})

    def write(self, slug):
        file = {"format": "coppir.exercise", "version": 1, "exported_at": T0, "exercise": self.ex}
        (OUT / f"{slug}.coppir").write_text(json.dumps(file, indent=1, ensure_ascii=False) + "\n")


# Trigger and effect shorthands
def at(minutes):
    return {"type": "at", "minutes": minutes}


def into(phase, minutes=0):
    return {"type": "phase", "phase": phase, "delay_minutes": minutes}


def when_status(asset, set, value):
    return {"type": "when", "condition": {"type": "asset_status", "asset": asset, "set": set, "value": value}}


def when_fired(inject):
    return {"type": "when", "condition": {"type": "inject_fired", "inject": inject}}


def when_poll(poll, option):
    return {"type": "when", "condition": {"type": "poll_choice", "poll": poll, "option": option}}


def when_breached(threshold):
    return {"type": "when", "condition": {"type": "threshold_breached", "threshold": threshold}}


def set_status(asset, set, value):
    return {"type": "set_status", "asset": asset, "set": set, "value": value}


def reveal(asset, *teams):
    vis = {"mode": "teams", "teams": list(teams)} if teams else {"mode": "all"}
    return {"type": "reveal_asset", "asset": asset, "visibility": vis}


def teams(*ids):
    return {"mode": "teams", "teams": list(ids)}


HIDDEN = {"mode": "facilitator"}

# ───────────────────────── 1. hospital ransomware ─────────────────────────


def ransomware():
    b = Builder(
        "Hospital ransomware",
        "A regional hospital's network is encrypted mid-shift. Clinical systems fail one after another while the "
        "teams decide whether to isolate the network, how to keep patients safe, and whether to pay. Topology view; "
        "about 100 minutes.",
        "EXERCISE — EXERCISE — EXERCISE",
    )
    b.types(
        ("identity", "Identity", "key-round", "#aa88ff"),
        ("server", "Server", "server", "#3399ff"),
        ("clinical", "Clinical system", "heart-pulse", "#ff6688"),
        ("endpoint", "Workstations", "monitor", "#888888"),
        ("network", "Network", "network", "#00ccaa"),
        ("backup", "Backup", "hard-drive", "#ffb000"),
        ("department", "Department", "hospital", "#ff6688"),
        ("threat", "Threat actor", "skull", "#ff3333"),
    )
    b.sector("it", "IT", "#3399ff", ["identity", "server", "endpoint", "network", "backup"])
    b.sector("clinical", "Clinical", "#ff6688", ["clinical", "department"])
    b.field("owner", "Owner", "text", player_visible=True)
    b.field("tier", "Criticality", "select", player_visible=True, options=["Tier 1", "Tier 2", "Tier 3"])
    b.field("rto", "Recovery time objective (hours)", "number", player_visible=True)
    b.field("ip", "IP range", "text")
    b.team("it", "IT & Security")
    b.team("clinical", "Clinical leadership")
    b.team("comms", "Legal & Comms")
    b.phases(("detect", "Detection", 15), ("contain", "Containment", 30), ("eradicate", "Eradication", 25),
             ("recover", "Recovery", 30))

    tier = lambda t, owner, rto, ip=None: {"tier": t, "owner": owner, "rto": rto, **({"ip": ip} if ip else {})}
    b.asset("fw", "Perimeter firewall", "network", at=(0, 0), fields=tier("Tier 1", "Network team", 2, "10.0.0.0/30"))
    b.asset("vpn", "Remote access VPN", "network", at=(250, 0), fields=tier("Tier 2", "Network team", 8, "10.0.1.0/24"))
    b.asset("ad", "Active Directory", "identity", at=(0, 160), fields=tier("Tier 1", "IT operations", 4, "10.1.0.10"))
    b.asset("ehr", "Electronic health records", "clinical", at=(-250, 320), fields=tier("Tier 1", "Clinical IT", 4))
    b.asset("pacs", "Imaging (PACS)", "clinical", at=(0, 320), fields=tier("Tier 1", "Radiology", 6))
    b.asset("lab", "Laboratory system", "clinical", at=(250, 320), fields=tier("Tier 2", "Pathology", 12))
    b.asset("ws", "Clinical workstations", "endpoint", at=(500, 160), fields=tier("Tier 2", "Desktop support", 24))
    b.asset("backup", "Backup server", "backup", at=(-500, 160), fields=tier("Tier 1", "IT operations", 2, "10.1.9.5"))
    b.asset("ed", "Emergency department", "department", at=(-250, 480))
    b.asset("radiology", "Radiology", "department", at=(0, 480))
    b.asset("wards", "Inpatient wards", "department", at=(250, 480))
    b.asset("c2", "Attacker command server", "threat", at=(250, -160), security="compromised", visible=HIDDEN,
            notes="Reached through a stolen VPN account. Reveal to IT once they find the beacon.")
    for provider, dependant in [("fw", "vpn"), ("fw", "ad"), ("ad", "ehr"), ("ad", "pacs"), ("ad", "lab"), ("ad", "ws"),
                                ("ehr", "ed"), ("ehr", "wards"), ("pacs", "radiology"), ("lab", "wards"), ("ad", "backup")]:
        b.link(provider, dependant)
    b.link("c2", "vpn", "stolen credentials", kind="connects_to")

    b.threshold("clinical-low", "Clinical systems failing", "clinical", 50)
    b.metric("beds", "Beds available", 80)
    b.metric("diversion", "Ambulances accepted", 100)
    b.poll("isolate", "Do you take the whole network offline now?",
           ["Isolate everything now", "Isolate only affected segments"], audience=teams("it", "clinical"), minutes=5)
    b.poll("pay", "The attackers want 50 BTC within 24 hours. Recommend paying?", ["Pay", "Refuse"],
           audience=teams("comms", "clinical"), minutes=8)

    b.inject("beacon", "Odd outbound traffic", "The SOC sees regular outbound connections from a VPN address to an "
             "unfamiliar host every 60 seconds.", severity="info", trigger=at(3), audience=teams("it"),
             effects=[set_status("vpn", "security", "under_investigation"), reveal("c2", "it")])
    b.inject("note", "Ransom notes on screens", "Workstations in the emergency department show a ransom note. Files "
             "are renamed with a .lockd extension.", severity="critical", trigger=into("contain"),
             effects=[set_status("ws", "security", "compromised"), set_status("ws", "operational", "offline"),
                      {"type": "open_poll", "poll": "isolate"}])
    b.inject("ad-hit", "Domain controller encrypted", "Active Directory is unreachable. Staff cannot log in.",
             severity="critical", trigger=when_poll("isolate", "isolate-1"),
             effects=[set_status("ad", "security", "compromised"), set_status("ad", "operational", "offline")],
             notes="The cautious path cost them the domain. Impact rules take EHR, PACS and the lab down with it.")
    b.inject("isolated", "Network isolated", "Everything is off the network. Clinical teams switch to paper.",
             trigger=when_poll("isolate", "isolate-0"),
             effects=[set_status("ehr", "operational", "degraded"), set_status("pacs", "operational", "degraded")])
    b.inject("divert", "Ambulance diversion", "The emergency department asks to divert ambulances to the next "
             "hospital.", severity="critical", trigger=when_breached("clinical-low"), audience=teams("clinical"))
    b.inject("backups", "Backups were encrypted too", "The backup server's volumes carry the same extension. The last "
             "clean copy is offline tape from last week.", severity="critical", trigger=into("eradicate", 5),
             effects=[set_status("backup", "security", "compromised"), {"type": "open_poll", "poll": "pay"}],
             notes="This is the pressure point for the payment decision.")
    b.inject("press", "Reporter calls", "A local reporter asks whether patients are being turned away.",
             severity="info", trigger=into("eradicate", 10), audience=teams("comms"))
    b.inject("paid", "Decryptor partly works", "The decryptor restores 60 % of files; EHR database files are "
             "corrupted.", trigger=when_poll("pay", "pay-0"), audience=teams("comms", "it"))
    b.inject("refused", "Data posted", "The attackers post a sample of patient records on their leak site.",
             severity="critical", trigger=when_poll("pay", "pay-1"), audience=teams("comms"))
    b.inject("restore", "EHR back from tape", "EHR is restored from tape. A week of records must be re-entered.",
             severity="info", trigger=into("recover", 10),
             effects=[set_status("ehr", "security", "monitored"), set_status("ehr", "operational", "degraded")])
    b.expect("beacon", ("it", "Open an incident and pull VPN logs for the address"),
             ("it", "Disable the VPN account involved"))
    b.expect("note", ("it", "Isolate affected segments from the network"),
             ("clinical", "Switch clinical units to downtime procedures"),
             ("comms", "Notify the executive on call and legal counsel"))
    b.expect("backups", ("it", "Check offline and tape backups before restoring"),
             ("clinical", "Decide which systems to restore first"))
    b.expect("press", ("comms", "Give the reporter a holding statement within 30 minutes"),
             (None, "Agree one spokesperson"))
    b.write("ransomware")


# ───────────────────────── 2. business email compromise ─────────────────────────


def bec():
    b = Builder(
        "Business email compromise",
        "A finance team receives an urgent payment request that looks like it comes from the CEO. The exercise "
        "follows the money, the mailbox and the vendor relationship. Topology view; about 75 minutes.",
        "EXERCISE — EXERCISE — EXERCISE",
    )
    b.types(
        ("person", "Person", "user", "#aa88ff"),
        ("mailbox", "Mailbox", "mail", "#3399ff"),
        ("idp", "Identity provider", "key-round", "#aa88ff"),
        ("finance", "Finance system", "banknote", "#33cc99"),
        ("bank", "Bank", "landmark", "#ffb000"),
        ("vendor", "Vendor", "truck", "#ff6688"),
        ("threat", "Threat actor", "skull", "#ff3333"),
    )
    b.sector("people", "People", "#aa88ff", ["person", "mailbox"])
    b.sector("money", "Payments", "#33cc99", ["finance", "bank", "vendor"])
    b.field("mfa", "MFA enabled", "bool", player_visible=True)
    b.field("limit", "Approval limit ($)", "number", player_visible=True)
    b.field("role", "Role", "text", player_visible=True)
    b.team("finance", "Finance")
    b.team("security", "IT Security")
    b.team("legal", "Legal")
    b.phases(("detect", "Detection", 15), ("respond", "Response", 30), ("recover", "Recovery", 30))

    b.asset("ceo", "Chief executive", "person", at=(0, 0), fields={"role": "CEO", "mfa": True})
    b.asset("cfo", "Finance director", "person", at=(300, 0), fields={"role": "CFO", "mfa": True, "limit": 250000})
    b.asset("ap", "Accounts payable clerk", "person", at=(600, 0), fields={"role": "AP", "mfa": False, "limit": 50000})
    b.asset("ceo-mail", "CEO mailbox", "mailbox", at=(0, 160), fields={"mfa": True})
    b.asset("cfo-mail", "CFO mailbox", "mailbox", at=(300, 160), fields={"mfa": True})
    b.asset("ap-mail", "AP mailbox", "mailbox", at=(600, 160), fields={"mfa": False})
    b.asset("idp", "Identity provider", "idp", at=(300, 320))
    b.asset("erp", "Payments system", "finance", at=(600, 320))
    b.asset("bank", "Company bank", "bank", at=(600, 480))
    b.asset("vendor", "Logistics supplier", "vendor", at=(900, 160))
    b.asset("lookalike", "Look-alike domain", "threat", at=(900, 0), security="compromised", visible=HIDDEN,
            notes="Registered two days ago; one letter differs from the vendor's domain.")
    for p, d in [("idp", "ceo-mail"), ("idp", "cfo-mail"), ("idp", "ap-mail"), ("ap", "erp"), ("erp", "bank")]:
        b.link(p, d)
    b.link("ceo", "ceo-mail", kind="managed_by")
    b.link("cfo", "cfo-mail", kind="managed_by")
    b.link("ap", "ap-mail", kind="managed_by")
    b.link("vendor", "ap-mail", "invoices", kind="connects_to")
    b.link("lookalike", "ap-mail", "spoofed invoices", kind="connects_to")

    b.metric("funds", "Funds recoverable", 100)
    b.threshold("money-low", "Payments compromised", "money", 60)
    b.poll("wire", "AP has a 'CEO-approved' request to wire $186,000 to a new account today. Approve it?",
           ["Approve the wire", "Hold and verify by phone"], audience=teams("finance"), minutes=4, text=False)

    b.inject("request", "Urgent request from the CEO", "\"I need this vendor payment out today, I'm in meetings, "
             "don't call.\" The vendor's bank details have changed.", trigger=at(2), audience=teams("finance"),
             effects=[{"type": "open_poll", "poll": "wire"}])
    b.inject("sent", "Wire sent", "$186,000 leaves the account to a new beneficiary.", severity="critical",
             trigger=when_poll("wire", "wire-0"),
             effects=[set_status("erp", "security", "compromised"), set_status("bank", "security", "under_investigation")])
    b.inject("held", "Verification call", "The CEO has never heard of the request.", severity="info",
             trigger=when_poll("wire", "wire-1"), audience=teams("finance", "security"),
             effects=[set_status("ap-mail", "security", "under_investigation")])
    b.inject("rule", "Hidden mailbox rule", "IT finds a rule in the AP mailbox that moves anything from the vendor "
             "to an RSS folder.", trigger=into("respond", 3), audience=teams("security"),
             effects=[set_status("ap-mail", "security", "compromised"), reveal("lookalike", "security")])
    b.inject("signin", "Sign-in from abroad", "The identity provider shows the AP clerk signing in from another "
             "country without MFA.", trigger=when_fired("rule"), audience=teams("security"),
             effects=[set_status("idp", "security", "under_investigation")])
    b.inject("recall", "Bank recall window", "The bank can try to recall the wire if asked within the hour.",
             trigger=when_fired("sent"), audience=teams("finance", "legal"))
    b.inject("vendor-call", "Vendor asks about late payment", "The real supplier says last month's invoice is "
             "unpaid and threatens to stop deliveries.", trigger=into("recover"), audience=teams("finance", "legal"))
    b.inject("regulator", "Notification deadline", "Counsel asks whether personal data in the mailbox triggers "
             "breach notification.", severity="info", trigger=into("recover", 10), audience=teams("legal"))
    b.expect("request", ("finance", "Verify the request by calling the CEO on a known number"),
             ("finance", "Hold the payment until verified"))
    b.expect("rule", ("security", "Remove the mailbox rule and reset the clerk's credentials"),
             ("security", "Search other mailboxes for the same rule"))
    b.expect("recall", ("finance", "Ask the bank to recall the wire within the hour"),
             ("legal", "Report the fraud to law enforcement"))
    b.expect("regulator", ("legal", "Decide whether the breach is notifiable, and record why"))
    b.write("bec")


# ───────────────────────── 3. supply-chain compromise ─────────────────────────


def supply_chain():
    b = Builder(
        "Supply-chain compromise",
        "A routine update from a trusted software vendor carries a backdoor into your monitoring platform. Teams "
        "must scope what the agent touched, decide whether to cut the vendor off, and answer customers. Topology "
        "view; about 90 minutes.",
        "EXERCISE — EXERCISE — EXERCISE",
    )
    b.types(
        ("vendor", "Vendor", "building-2", "#ff6688"),
        ("pipeline", "Build system", "git-branch", "#ff6688"),
        ("update", "Update server", "cloud-download", "#ff6688"),
        ("platform", "Monitoring platform", "activity", "#3399ff"),
        ("server", "Server", "server", "#3399ff"),
        ("data", "Data store", "database", "#ffb000"),
        ("customer", "Customer", "users", "#33cc99"),
        ("threat", "Threat actor", "skull", "#ff3333"),
    )
    b.sector("vendor", "Vendor", "#ff6688", ["vendor", "pipeline", "update"])
    b.sector("estate", "Our estate", "#3399ff", ["platform", "server", "data"])
    b.sector("customers", "Customers", "#33cc99", ["customer"])
    b.field("version", "Agent version", "text", player_visible=True)
    b.field("records", "Customer records", "number")
    b.field("sla", "Contract SLA", "select", player_visible=True, options=["Gold", "Silver", "Bronze"])
    b.team("soc", "Security operations")
    b.team("eng", "Engineering")
    b.team("cx", "Customer & Legal")
    b.phases(("detect", "Detection", 20), ("scope", "Scoping", 25), ("contain", "Containment", 25),
             ("recover", "Recovery", 20))

    b.asset("vendor", "Vendor HQ", "vendor", at=(0, 0))
    b.asset("build", "Vendor build pipeline", "pipeline", at=(0, 160), security="compromised", visible=HIDDEN,
            notes="The attacker has been inside the vendor's build system for five months.")
    b.asset("updates", "Vendor update server", "update", at=(0, 320))
    b.asset("monitor", "Monitoring platform", "platform", at=(300, 320), fields={"version": "2026.4.1"})
    b.asset("web", "Web servers", "server", at=(600, 160), fields={"version": "2026.4.1"})
    b.asset("api", "API servers", "server", at=(600, 320), fields={"version": "2026.4.1"})
    b.asset("db", "Customer database", "data", at=(600, 480), fields={"records": 2400000})
    b.asset("dw", "Analytics warehouse", "data", at=(900, 480), fields={"records": 9100000})
    b.asset("acme", "Northwind Retail", "customer", at=(900, 160), fields={"sla": "Gold"})
    b.asset("globex", "Contoso Health", "customer", at=(900, 320), fields={"sla": "Gold"})
    b.asset("initech", "Fabrikam Logistics", "customer", at=(1200, 320), fields={"sla": "Silver"})
    b.asset("c2", "Attacker infrastructure", "threat", at=(-300, 320), security="compromised", visible=HIDDEN)
    for p, d in [("vendor", "build"), ("build", "updates"), ("updates", "monitor"), ("monitor", "web"),
                 ("monitor", "api"), ("api", "db"), ("db", "dw"), ("api", "acme"), ("api", "globex"), ("api", "initech")]:
        b.link(p, d)
    b.link("c2", "monitor", "agent beacon", kind="connects_to")

    b.threshold("customers-hit", "Customer impact", "customers", 70, severity="warning")
    b.metric("confidence", "Scoping confidence", 20)
    b.poll("cutoff", "Do you remove the vendor's agent from every server now, losing monitoring?",
           ["Remove it everywhere", "Keep it, block its network traffic"], audience=teams("soc", "eng"), minutes=6)

    b.inject("advisory", "Vendor security advisory", "The vendor says version 2026.4.1 'may contain unauthorised "
             "code'. No details yet.", trigger=at(4), effects=[set_status("updates", "security", "under_investigation")])
    b.inject("beacon", "Agent talks to an odd host", "The monitoring agent makes DNS requests to a domain registered "
             "last month.", trigger=into("scope", 2), audience=teams("soc"),
             effects=[set_status("monitor", "security", "compromised"), reveal("c2", "soc"),
                      {"type": "open_poll", "poll": "cutoff"}])
    b.inject("removed", "Agent removed", "Agents are gone; so is visibility into the estate.",
             trigger=when_poll("cutoff", "cutoff-0"), audience=teams("soc", "eng"),
             effects=[set_status("monitor", "operational", "offline"), set_status("monitor", "security", "contained")])
    b.inject("blocked", "Egress blocked", "The agent stays, its outbound traffic is blocked. The attacker tries other "
             "ports.", trigger=when_poll("cutoff", "cutoff-1"), audience=teams("soc"),
             effects=[set_status("api", "security", "under_investigation")])
    b.inject("db-touch", "Queries against customer data", "Database logs show bulk SELECTs from the agent's service "
             "account at 03:00 on three nights.", severity="critical", trigger=into("contain"), audience=teams("soc"),
             effects=[set_status("db", "security", "compromised")])
    b.inject("customer", "A customer asks", "Contoso Health's CISO asks whether their data was exposed and wants an "
             "answer by end of day.", trigger=when_fired("db-touch"), audience=teams("cx"),
             effects=[set_status("globex", "security", "under_investigation")])
    b.inject("vendor-truth", "Vendor's build was breached", "The vendor confirms its build pipeline was compromised "
             "for months.", severity="info", trigger=into("contain", 15), effects=[reveal("build")])
    b.inject("clean-build", "Clean version published", "The vendor publishes 2026.4.3, rebuilt from a clean pipeline.",
             severity="info", trigger=into("recover", 5), effects=[set_status("updates", "security", "monitored")])
    b.expect("advisory", ("soc", "Inventory every host running the affected version"),
             ("eng", "Freeze updates from the vendor"))
    b.expect("beacon", ("soc", "Block the domain at DNS and the proxy"),
             ("soc", "Preserve agent logs before changing anything"))
    b.expect("db-touch", ("eng", "Rotate the agent's service account credentials"),
             ("cx", "Start a list of customers whose data may be affected"))
    b.expect("customer", ("cx", "Reply with what is known, and when the next update will come"),
             (None, "Agree what may be shared under NDA"))
    b.write("supply-chain")


# ───────────────────────── 4. regional utility outage ─────────────────────────


def utility_outage():
    b = Builder(
        "Regional utility outage",
        "An intrusion into a utility's control systems coincides with a heat wave. Substations trip, and hospitals, "
        "water and transit feel it. Map view of a fictional county; about 100 minutes.",
        "EXERCISE — EXERCISE — EXERCISE",
    )
    b.types(
        ("substation", "Substation", "zap", "#ffdd00"),
        ("control", "Control centre", "monitor-cog", "#ffdd00"),
        ("hospital", "Hospital", "hospital", "#3399ff"),
        ("water", "Water plant", "droplet", "#00ccaa"),
        ("telecom", "Telecom site", "radio-tower", "#00ccaa"),
        ("transit", "Transit hub", "train-front", "#00ccaa"),
        ("responder", "Emergency services", "siren", "#ff4444"),
    )
    b.sector("power", "Power", "#ffdd00", ["substation", "control"])
    b.sector("medical", "Medical", "#3399ff", ["hospital"])
    b.sector("lifeline", "Lifelines", "#00ccaa", ["water", "telecom", "transit"])
    b.sector("emergency", "Emergency", "#ff4444", ["responder"])
    b.field("generator", "Backup generator (hours)", "number", player_visible=True)
    b.field("customers", "Customers served", "number", player_visible=True)
    b.field("scada", "SCADA vendor", "text")
    b.team("utility", "Utility")
    b.team("county", "County emergency management")
    b.team("health", "Health & water")
    b.phases(("detect", "Detection", 20), ("respond", "Response", 35), ("restore", "Restoration", 30),
             ("review", "Hot wash", 15))

    # A fictional county laid over open country, so nothing real is implied.
    base = (44.06, -121.31)
    g = lambda dlat, dlon: (round(base[0] + dlat, 4), round(base[1] + dlon, 4))
    b.asset("ecc", "Energy control centre", "control", geo=g(0.00, 0.00), fields={"scada": "GridLink 9"})
    b.asset("north", "North substation", "substation", geo=g(0.06, 0.02), fields={"customers": 41000})
    b.asset("river", "River substation", "substation", geo=g(-0.01, 0.07), fields={"customers": 38000})
    b.asset("south", "South substation", "substation", geo=g(-0.07, -0.01), fields={"customers": 52000})
    b.asset("mercy", "Mercy General Hospital", "hospital", geo=g(0.05, 0.05), fields={"generator": 48})
    b.asset("valley", "Valley Medical Centre", "hospital", geo=g(-0.06, 0.03), fields={"generator": 12})
    b.asset("plant", "Water treatment plant", "water", geo=g(0.01, 0.10), fields={"generator": 6})
    b.asset("tower", "Hilltop telecom site", "telecom", geo=g(0.08, -0.05), fields={"generator": 8})
    b.asset("depot", "Central transit hub", "transit", geo=g(-0.02, 0.01))
    b.asset("eoc", "County emergency operations", "responder", geo=g(0.02, -0.03))
    b.asset("fire", "Fire station 3", "responder", geo=g(-0.04, 0.06))
    b.asset("rtu", "Compromised remote terminal", "control", geo=g(-0.05, -0.04), security="compromised",
            visible=HIDDEN, notes="Field device on the south feeder, reached from a contractor laptop.")
    for p, d in [("ecc", "north"), ("ecc", "river"), ("ecc", "south"), ("north", "mercy"), ("north", "tower"),
                 ("river", "plant"), ("river", "depot"), ("south", "valley"), ("south", "fire"), ("tower", "eoc")]:
        b.link(p, d)
    b.link("rtu", "south", "malicious commands", kind="connects_to")

    b.threshold("power-crit", "Grid critical", "power", 40)
    b.threshold("medical-low", "Hospitals at risk", "medical", 50, severity="warning")
    b.metric("temp", "Heat index (% of record)", 92)
    b.metric("crews", "Line crews available", 70)
    b.poll("shed", "Load must come off the grid. Where do you shed first?",
           ["Residential rotating outages", "Industrial customers", "Protect everyone, risk a wider trip"],
           audience=teams("utility", "county"), minutes=5)

    b.inject("trip", "South substation trips", "Breakers at South open without an operator command. 52,000 "
             "customers lose power in 38 °C heat.", severity="critical", trigger=at(5),
             effects=[set_status("south", "operational", "offline"), set_status("south", "security", "under_investigation")])
    b.inject("scada", "Unrecognised commands", "Control centre logs show breaker commands from a field terminal "
             "nobody was using.", trigger=into("respond", 2), audience=teams("utility"),
             effects=[set_status("ecc", "security", "under_investigation"), reveal("rtu", "utility"),
                      {"type": "open_poll", "poll": "shed"}])
    b.inject("wider", "Cascading trip", "Without shedding load, River substation overloads and trips.",
             severity="critical", trigger=when_poll("shed", "shed-2"),
             effects=[set_status("river", "operational", "offline")])
    b.inject("rotating", "Rotating outages begin", "Residential feeders rotate off in 30-minute blocks. Complaints "
             "flood the call centre.", trigger=when_poll("shed", "shed-0"), audience=teams("utility", "county"),
             effects=[set_status("north", "operational", "degraded")])
    b.inject("industrial", "Industrial load shed", "Two plants shut down; one warns of a chemical process upset.",
             trigger=when_poll("shed", "shed-1"), audience=teams("county"))
    b.inject("generator", "Valley Medical on generator", "Valley Medical has 12 hours of fuel and asks for priority "
             "delivery.", trigger=when_status("valley", "operational", "degraded"), audience=teams("health", "county"))
    b.inject("boil", "Boil-water decision", "Plant pressure is falling; the health officer must decide on a "
             "boil-water notice.", severity="warning", trigger=when_status("plant", "operational", "degraded"),
             audience=teams("health"))
    b.inject("medical", "Hospitals at risk", "Two hospitals are on backup power with limited fuel.",
             trigger=when_breached("medical-low"), audience=teams("county", "health"))
    b.inject("restore", "Crews restore South", "Crews isolate the terminal and close South's breakers by hand.",
             severity="info", trigger=into("restore", 10),
             effects=[set_status("south", "operational", "degraded"), set_status("south", "security", "contained")])
    b.expect("trip", ("utility", "Dispatch crews to South substation"),
             ("county", "Open the emergency operations centre"))
    b.expect("scada", ("utility", "Disconnect the field terminal and switch to manual control"),
             ("utility", "Report the intrusion to the national CERT"))
    b.expect("generator", ("health", "Arrange fuel deliveries for hospitals on generator"),
             ("county", "Set priority restoration for critical facilities"))
    b.expect("boil", ("health", "Decide on a boil-water notice and issue it"),
             (None, "Publish one consistent public message"))
    b.write("utility-outage")


if __name__ == "__main__":
    ransomware()
    bec()
    supply_chain()
    utility_outage()
    print("wrote", ", ".join(p.name for p in sorted(OUT.glob("*.coppir"))))
