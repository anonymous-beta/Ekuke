#!/usr/bin/env python3
import json, sys, socket, concurrent.futures

DEFAULT_WORDLIST = [
    "www","mail","ftp","localhost","admin","portal","test","dev","staging","api","app","blog",
    "shop","forum","support","cdn","media","static","secure","vpn","remote","webmail","ns1","ns2",
    "mx","smtp","pop","imap","cpanel","webdisk","whm","autodiscover","m","mobile","beta","alpha",
    "demo","old","new","v1","v2","v3","api-v1","api-v2","graphql","rest","ws","websocket","git",
    "svn","jenkins","gitlab","github","confluence","jira","wiki","docs","help","status","monitor",
    "grafana","prometheus","kibana","elastic","search","db","database","sql","mysql","postgres",
    "mongo","redis","backup","bak","archive","temp","tmp","cache","img","images","css","js",
    "assets","uploads","files","data","download","downloads","share","public","private","internal",
    "intranet","corp","enterprise","business","partner","vendor","client","customer","user",
    "account","auth","login","logon","signin","signup","register","sso","oauth","ad","ldap","dc",
    "exchange","sharepoint","teams","office","azure","aws","cloud","s3","bucket","storage","drive",
    "www2","www1","host","server","gateway","firewall","router","switch","ap","wireless","print",
    "printer","scanner","camera","dvr","nvr","iot","hub","bridge","gateway","controller"
]

def resolve(subdomain):
    try:
        socket.gethostbyname(subdomain)
        return subdomain
    except:
        return None

def main():
    data = json.load(sys.stdin)
    domain = data["entity"]["label"]
    config = data.get("config", {})
    wordlist_path = config.get("wordlist", "")
    threads = int(config.get("threads", "50"))

    words = DEFAULT_WORDLIST
    if wordlist_path:
        try:
            with open(wordlist_path) as f:
                words = [l.strip() for l in f if l.strip() and not l.startswith("#")]
        except Exception as e:
            print(json.dumps({"error": f"Wordlist error: {e}"}))
            return

    targets = [f"{w}.{domain}" for w in words]
    found = []

    with concurrent.futures.ThreadPoolExecutor(max_workers=threads) as ex:
        futures = {ex.submit(resolve, t): t for t in targets}
        for future in concurrent.futures.as_completed(futures):
            result = future.result()
            if result:
                found.append(result)

    entities = []
    relationships = []
    for sub in sorted(set(found)):
        entities.append({
            "entity_type": "Domain",
            "label": sub,
            "properties": {"source": "subdomain-bruteforce", "parent_domain": domain}
        })
        relationships.append({
            "rel_type": "SUBDOMAIN_OF",
            "source_label": sub,
            "source_type": "Domain",
            "target_label": domain,
            "target_type": "Domain",
            "properties": {}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
