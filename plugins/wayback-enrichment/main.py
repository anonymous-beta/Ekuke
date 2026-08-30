#!/usr/bin/env python3
import json, sys, requests
from urllib.parse import urlparse

def main():
    data = json.load(sys.stdin)
    target = data["entity"]["label"]
    config = data.get("config", {})
    proxy = config.get("proxy", "")
    limit = int(config.get("limit", "1000"))
    proxies = {"http": proxy, "https": proxy} if proxy else None

    if data["entity"]["entity_type"] == "URL":
        parsed = urlparse(target)
        target = parsed.netloc or target

    try:
        r = requests.get(
            "http://web.archive.org/cdx/search/cdx",
            params={
                "url": f"{target}/*",
                "output": "json",
                "fl": "original,timestamp,mimetype,statuscode",
                "collapse": "urlkey",
                "limit": limit
            },
            timeout=60, proxies=proxies
        )
        r.raise_for_status()
        results = r.json()
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        return

    if not results or len(results) < 2:
        print(json.dumps({"entities": [], "relationships": []}))
        return

    entities = []
    relationships = []
    seen = set()

    for row in results[1:]:
        if len(row) < 2:
            continue
        url = row[0]
        timestamp = row[1]
        mimetype = row[2] if len(row) > 2 else ""
        status = row[3] if len(row) > 3 else ""
        archive_url = f"https://web.archive.org/web/{timestamp}/{url}"

        if url in seen:
            continue
        seen.add(url)

        entities.append({
            "entity_type": "URL",
            "label": url,
            "properties": {
                "archive_url": archive_url,
                "archive_date": timestamp,
                "mimetype": mimetype,
                "status_code": status,
                "source": "wayback-machine"
            }
        })
        relationships.append({
            "rel_type": "ARCHIVED_VERSION",
            "source_label": target,
            "source_type": "Domain",
            "target_label": url,
            "target_type": "URL",
            "properties": {"snapshot_date": timestamp}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
