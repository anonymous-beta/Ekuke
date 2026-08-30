#!/usr/bin/env python3
import json, sys, requests, socket

def main():
    data = json.load(sys.stdin)
    entity = data["entity"]
    config = data.get("config", {})
    api_key = config.get("api_key", "")
    proxy = config.get("proxy", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    if not api_key:
        print(json.dumps({"error": "Shodan API key required"}))
        return

    target = entity["label"]
    if entity["entity_type"] == "Domain":
        try:
            target = socket.gethostbyname(target)
        except Exception as e:
            print(json.dumps({"error": f"Resolve failed: {e}"}))
            return

    try:
        r = requests.get(
            f"https://api.shodan.io/shodan/host/{target}?key={api_key}",
            timeout=30, proxies=proxies
        )
        r.raise_for_status()
        host = r.json()
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        return

    entities = []
    relationships = []
    ip_str = host.get("ip_str", target)

    entities.append({
        "entity_type": "IPv4",
        "label": ip_str,
        "properties": {
            "shodan_os": host.get("os", ""),
            "shodan_org": host.get("org", ""),
            "shodan_isp": host.get("isp", ""),
            "shodan_asn": host.get("asn", ""),
            "shodan_tags": host.get("tags", []),
            "shodan_hostnames": host.get("hostnames", []),
            "shodan_country": host.get("country_name", ""),
            "shodan_city": host.get("city", ""),
            "shodan_region": host.get("region_code", ""),
            "shodan_latitude": host.get("latitude"),
            "shodan_longitude": host.get("longitude"),
            "source": "shodan"
        }
    })

    for item in host.get("data", []):
        port = item.get("port")
        product = item.get("product", "")
        version = item.get("version", "")
        banner = item.get("data", "")[:800]
        port_label = f"{ip_str}:{port}"

        entities.append({
            "entity_type": "Port",
            "label": port_label,
            "properties": {
                "port": port,
                "product": product,
                "version": version,
                "banner": banner,
                "transport": item.get("transport", "tcp"),
                "source": "shodan"
            }
        })
        relationships.append({
            "rel_type": "HAS_OPEN_PORT",
            "source_label": ip_str,
            "source_type": "IPv4",
            "target_label": port_label,
            "target_type": "Port",
            "properties": {"service": product}
        })

        if item.get("ssl"):
            cert = item["ssl"].get("cert", {})
            subj = cert.get("subject", {})
            cn = subj.get("CN", "") if isinstance(subj, dict) else ""
            if cn:
                cert_label = f"SSL:{cn}"
                entities.append({
                    "entity_type": "Certificate",
                    "label": cert_label,
                    "properties": {
                        "issuer": str(cert.get("issuer", "")),
                        "subject": cn,
                        "fingerprint": cert.get("fingerprint", ""),
                        "serial": str(cert.get("serial", "")),
                        "source": "shodan"
                    }
                })
                relationships.append({
                    "rel_type": "USES_CERTIFICATE",
                    "source_label": port_label,
                    "source_type": "Port",
                    "target_label": cert_label,
                    "target_type": "Certificate",
                    "properties": {}
                })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
