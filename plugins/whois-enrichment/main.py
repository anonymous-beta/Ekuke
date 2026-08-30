#!/usr/bin/env python3
import json, sys, subprocess, re

def main():
    data = json.load(sys.stdin)
    entity = data["entity"]
    domain = entity["label"]
    if entity["entity_type"] == "URL":
        from urllib.parse import urlparse
        domain = urlparse(domain).netloc or domain

    raw = ""
    try:
        import whois
        w = whois.whois(domain)
        raw = str(w)
        registrar = w.registrar if isinstance(w.registrar, str) else (w.registrar[0] if isinstance(w.registrar, list) else "")
        creation = str(w.creation_date[0]) if isinstance(w.creation_date, list) else str(w.creation_date)
        expiration = str(w.expiration_date[0]) if isinstance(w.expiration_date, list) else str(w.expiration_date)
        updated = str(w.updated_date[0]) if isinstance(w.updated_date, list) else str(w.updated_date)
        name_servers = w.name_servers if isinstance(w.name_servers, list) else ([w.name_servers] if w.name_servers else [])
        org = w.org if isinstance(w.org, str) else ""
        emails = w.emails if isinstance(w.emails, list) else ([w.emails] if w.emails else [])
    except Exception:
        try:
            raw = subprocess.check_output(["whois", domain], text=True, timeout=15)
            registrar = re.search(r"Registrar:\s*(.+)", raw)
            registrar = registrar.group(1).strip() if registrar else ""
            creation = re.search(r"Creation Date:\s*(.+)", raw)
            creation = creation.group(1).strip() if creation else ""
            expiration = re.search(r"Registry Expiry Date:\s*(.+)", raw)
            expiration = expiration.group(1).strip() if expiration else ""
            updated = re.search(r"Updated Date:\s*(.+)", raw)
            updated = updated.group(1).strip() if updated else ""
            name_servers = list(set(re.findall(r"Name Server:\s*(.+)", raw)))
            org = re.search(r"Registrant Organization:\s*(.+)", raw)
            org = org.group(1).strip() if org else ""
            emails = list(set(re.findall(r"[\w\.-]+@[\w\.-]+\.\w+", raw)))
        except Exception as e:
            print(json.dumps({"error": str(e)}))
            return

    entities = []
    relationships = []

    for ns in name_servers:
        ns_clean = ns.strip().lower().rstrip('.')
        if not ns_clean:
            continue
        entities.append({
            "entity_type": "Domain",
            "label": ns_clean,
            "properties": {"role": "name_server", "source": "whois"}
        })
        relationships.append({
            "rel_type": "HAS_NAME_SERVER",
            "source_label": domain,
            "source_type": "Domain",
            "target_label": ns_clean,
            "target_type": "Domain",
            "properties": {}
        })

    for email in emails:
        entities.append({
            "entity_type": "Email",
            "label": email,
            "properties": {"source": "whois", "role": "registrant"}
        })
        relationships.append({
            "rel_type": "REGISTRANT_EMAIL",
            "source_label": domain,
            "source_type": "Domain",
            "target_label": email,
            "target_type": "Email",
            "properties": {}
        })

    if org:
        entities.append({
            "entity_type": "Organization",
            "label": org,
            "properties": {"source": "whois", "role": "registrant"}
        })
        relationships.append({
            "rel_type": "REGISTRANT_ORG",
            "source_label": domain,
            "source_type": "Domain",
            "target_label": org,
            "target_type": "Organization",
            "properties": {}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
