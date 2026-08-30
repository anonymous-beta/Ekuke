#!/usr/bin/env python3
import json, sys, requests

def main():
    data = json.load(sys.stdin)
    url = data["entity"]["label"]
    config = data.get("config", {})
    proxy = config.get("proxy", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    try:
        r = requests.head(url, allow_redirects=True, timeout=15, proxies=proxies, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        })
        chain = []
        for resp in r.history + [r]:
            chain.append({"url": resp.url, "status": resp.status_code})

        entities = []
        relationships = []

        for i in range(1, len(chain)):
            step = chain[i]
            prev = chain[i-1]
            entities.append({
                "entity_type": "URL",
                "label": step["url"],
                "properties": {
                    "status_code": step["status"],
                    "source": "url-unshortener",
                    "redirect_index": i
                }
            })
            relationships.append({
                "rel_type": "REDIRECTS_TO",
                "source_label": prev["url"],
                "source_type": "URL",
                "target_label": step["url"],
                "target_type": "URL",
                "properties": {"status_code": step["status"]}
            })

        print(json.dumps({"entities": entities, "relationships": relationships}))
    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    main()
