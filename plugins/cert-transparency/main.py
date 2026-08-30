#!/usr/bin/env python3
import json, sys, requests

def main():
    data = json.load(sys.stdin)
    domain = data["entity"]["label"]
    config = data.get("config", {})
    proxy = config.get("proxy", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    try:
        r = requests.get(f"https://crt.sh/?q=%.{domain}&output=json", timeout=45, proxies=proxies)
        r.raise_for_status()
        results = r.json()
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        return

    seen = set()
    entities = []
    relationships = []
    issuers = {}

    for entry in results:
        name = entry.get("name_value", "").strip()
        issuer = entry.get("issuer_name", "").strip()
        if not name:
            continue
        for sub in name.split("\n"):
            sub = sub.strip().lower().rstrip('.')
            if not sub or sub in seen or sub == domain or "*" in sub:
                continue
            seen.add(sub)
            entities.append({
                "entity_type": "Domain",
                "label": sub,
                "properties": {"source": "certificate-transparency", "issuer": issuer}
            })
            relationships.append({
                "rel_type": "CERT_SUBDOMAIN",
                "source_label": domain,
                "source_type": "Domain",
                "target_label": sub,
                "target_type": "Domain",
                "properties": {"issuer": issuer}
            })
            if issuer:
                issuers[issuer] = issuers.get(issuer, 0) + 1

    for issuer, count in issuers.items():
        org_name = issuer.split("=")[-1] if "=" in issuer else issuer
        entities.append({
            "entity_type": "Organization",
            "label": org_name,
            "properties": {"role": "Certificate Issuer", "count": count, "source": "certificate-transparency"}
        })
        relationships.append({
            "rel_type": "ISSUED_BY",
            "source_label": domain,
            "source_type": "Domain",
            "target_label": org_name,
            "target_type": "Organization",
            "properties": {}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
