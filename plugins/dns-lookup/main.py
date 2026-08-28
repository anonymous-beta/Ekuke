#!/usr/bin/env python3
import json
import sys
import socket

def main():
    input_data = json.load(sys.stdin)
    entity = input_data["entity"]
    config = input_data.get("config", {})
    record_type = config.get("record_type", "A").upper()
    domain = entity["label"]
    
    entities = []
    relationships = []
    
    try:
        if record_type == "A":
            answers = socket.getaddrinfo(domain, None, socket.AF_INET)
            seen = set()
            for ans in answers:
                ip = ans[4][0]
                if ip not in seen:
                    seen.add(ip)
                    entities.append({
                        "entity_type": "IPv4",
                        "label": ip,
                        "properties": {"source": "dns-lookup", "record_type": "A"}
                    })
                    relationships.append({
                        "rel_type": "RESOLVES_TO",
                        "source_label": domain,
                        "source_type": "Domain",
                        "target_label": ip,
                        "target_type": "IPv4",
                        "properties": {}
                    })
        else:
            try:
                import dns.resolver
                answers = dns.resolver.resolve(domain, record_type)
                for rdata in answers:
                    val = str(rdata)
                    if record_type in ("A", "AAAA"):
                        target_type = "IPv4"
                    elif record_type == "CNAME":
                        target_type = "Domain"
                    elif record_type == "MX":
                        target_type = "Domain"
                        val = str(rdata.exchange).rstrip('.')
                    elif record_type == "NS":
                        target_type = "Domain"
                        val = str(rdata).rstrip('.')
                    else:
                        target_type = "Other"
                    
                    entities.append({
                        "entity_type": target_type,
                        "label": val,
                        "properties": {"source": "dns-lookup", "record_type": record_type, "raw": str(rdata)}
                    })
                    relationships.append({
                        "rel_type": "HAS_RECORD",
                        "source_label": domain,
                        "source_type": "Domain",
                        "target_label": val,
                        "target_type": target_type,
                        "properties": {"record_type": record_type}
                    })
            except ImportError:
                return print(json.dumps({"error": "dnspython required for non-A records. pip install dnspython"}))
    except Exception as e:
        return print(json.dumps({"error": str(e)}))
    
    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
