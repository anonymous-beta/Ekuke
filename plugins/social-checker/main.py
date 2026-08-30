#!/usr/bin/env python3
import json, sys, requests

PLATFORMS = {
    "Twitter/X": "https://x.com/{}",
    "GitHub": "https://github.com/{}",
    "Instagram": "https://www.instagram.com/{}/",
    "Reddit": "https://www.reddit.com/user/{}",
    "TikTok": "https://www.tiktok.com/@{}",
    "YouTube": "https://www.youtube.com/@{}",
    "LinkedIn": "https://www.linkedin.com/in/{}",
    "Twitch": "https://www.twitch.tv/{}",
    "Steam": "https://steamcommunity.com/id/{}",
    "GitLab": "https://gitlab.com/{}",
    "Medium": "https://medium.com/@{}",
    "Dev.to": "https://dev.to/{}",
    "Pinterest": "https://www.pinterest.com/{}/",
}

def check_platform(handle, url_template, proxies):
    try:
        url = url_template.format(handle)
        r = requests.get(url, timeout=10, allow_redirects=True, proxies=proxies, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.0"
        })
        if r.status_code != 200:
            return None
        content = r.text.lower()
        indicators = ["page not found","not found","doesn't exist","does not exist",
                     "sorry, nobody on reddit goes by","this page is unavailable",
                     "user not found","profile not found","suspended","404"]
        if any(ind in content for ind in indicators):
            return None
        return url
    except:
        return None

def main():
    data = json.load(sys.stdin)
    handle = data["entity"]["label"].strip().lstrip("@")
    config = data.get("config", {})
    proxy = config.get("proxy", "")
    platforms_str = config.get("platforms", "")
    proxies = {"http": proxy, "https": proxy} if proxy else None

    check_list = PLATFORMS
    if platforms_str:
        selected = [p.strip() for p in platforms_str.split(",")]
        check_list = {k: v for k, v in PLATFORMS.items() if k in selected}

    entities = []
    relationships = []

    for name, template in check_list.items():
        url = check_platform(handle, template, proxies)
        if url:
            label = f"{handle} ({name})"
            entities.append({
                "entity_type": "Profile",
                "label": label,
                "properties": {
                    "platform": name,
                    "username": handle,
                    "url": url,
                    "source": "social-checker"
                }
            })
            relationships.append({
                "rel_type": "HAS_PROFILE",
                "source_label": handle,
                "source_type": "Handle",
                "target_label": label,
                "target_type": "Profile",
                "properties": {"platform": name, "url": url}
            })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
