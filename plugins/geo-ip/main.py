#!/usr/bin/env python3
import json, sys, requests

def main():
    data = json.load(sys.stdin)
    ip = data["entity"]["label"]
    config = data.get("config", {})
    proxy = config.get("proxy", "")
    local_db = config.get("local_db", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    try:
        if local_db:
            import geoip2.database
            reader = geoip2.database.Reader(local_db)
            rec = reader.city(ip)
            geo = {
                "country": rec.country.name or "",
                "countryCode": rec.country.iso_code or "",
                "city": rec.city.name or "",
                "zip": rec.postal.code or "",
                "lat": rec.location.latitude,
                "lon": rec.location.longitude,
                "isp": "",
                "org": rec.traits.organization or "",
                "as": "",
                "proxy": rec.traits.is_anonymous_proxy or False,
                "hosting": rec.traits.is_anonymous_proxy or False
            }
        else:
            r = requests.get(
                f"http://ip-api.com/json/{ip}?fields=status,message,country,countryCode,region,regionName,city,zip,lat,lon,timezone,isp,org,as,asname,mobile,proxy,hosting",
                timeout=10, proxies=proxies
            )
            r.raise_for_status()
            geo = r.json()
            if geo.get("status") != "success":
                print(json.dumps({"error": geo.get("message", "ip-api failed")}))
                return
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        return

    entities = []
    relationships = []

    loc_label = f"{geo.get('city','Unknown')}, {geo.get('country','Unknown')}"
    entities.append({
        "entity_type": "Location",
        "label": loc_label,
        "properties": {
            "country": geo.get("country", ""),
            "country_code": geo.get("countryCode", ""),
            "region": geo.get("regionName", ""),
            "city": geo.get("city", ""),
            "zip": geo.get("zip", ""),
            "latitude": geo.get("lat"),
            "longitude": geo.get("lon"),
            "timezone": geo.get("timezone", ""),
            "source": "geo-ip"
        }
    })
    relationships.append({
        "rel_type": "LOCATED_IN",
        "source_label": ip,
        "source_type": "IPv4",
        "target_label": loc_label,
        "target_type": "Location",
        "properties": {}
    })

    isp = geo.get("isp", "") or geo.get("org", "")
    if isp:
        entities.append({
            "entity_type": "Organization",
            "label": isp,
            "properties": {"role": "ISP", "asn": geo.get("as", ""), "asn_name": geo.get("asname", ""), "source": "geo-ip"}
        })
        relationships.append({
            "rel_type": "ASSIGNED_TO",
            "source_label": ip,
            "source_type": "IPv4",
            "target_label": isp,
            "target_type": "Organization",
            "properties": {}
        })

    if geo.get("proxy"):
        entities.append({
            "entity_type": "Other",
            "label": f"Proxy/VPN ({ip})",
            "properties": {"indicator": "proxy", "source": "geo-ip"}
        })
        relationships.append({
            "rel_type": "IDENTIFIED_AS",
            "source_label": ip,
            "source_type": "IPv4",
            "target_label": f"Proxy/VPN ({ip})",
            "target_type": "Other",
            "properties": {}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
