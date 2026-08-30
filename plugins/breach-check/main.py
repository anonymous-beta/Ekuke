#!/usr/bin/env python3
import json, sys, requests

def main():
    data = json.load(sys.stdin)
    email = data["entity"]["label"]
    config = data.get("config", {})
    api_key = config.get("hibp_api_key", "")
    proxy = config.get("proxy", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    if not api_key:
        print(json.dumps({"error": "HaveIBeenPwned API key required"}))
        return

    headers = {
        "hibp-api-key": api_key,
        "user-agent": "EKUKE-OSINT-Tool"
    }

    try:
        r = requests.get(
            f"https://haveibeenpwned.com/api/v3/breachedaccount/{email}",
            headers=headers, timeout=30, proxies=proxies
        )
        if r.status_code == 404:
            print(json.dumps({"entities": [], "relationships": []}))
            return
        r.raise_for_status()
        breaches = r.json()
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        return

    entities = []
    relationships = []

    for breach in breaches:
        name = breach.get("Name", "Unknown")
        entities.append({
            "entity_type": "Breach",
            "label": name,
            "properties": {
                "title": breach.get("Title", ""),
                "domain": breach.get("Domain", ""),
                "breach_date": breach.get("BreachDate", ""),
                "added_date": breach.get("AddedDate", ""),
                "pwn_count": breach.get("PwnCount", 0),
                "description": breach.get("Description", ""),
                "data_classes": breach.get("DataClasses", []),
                "is_verified": breach.get("IsVerified", False),
                "is_fabricated": breach.get("IsFabricated", False),
                "is_sensitive": breach.get("IsSensitive", False),
                "is_retired": breach.get("IsRetired", False),
                "is_spam_list": breach.get("IsSpamList", False),
                "source": "haveibeenpwned"
            }
        })
        relationships.append({
            "rel_type": "COMPROMISED_IN",
            "source_label": email,
            "source_type": "Email",
            "target_label": name,
            "target_type": "Breach",
            "properties": {}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
