#!/bin/bash
set -e
PLUGIN_DIR="${HOME}/.ekuke/plugins"
mkdir -p "$PLUGIN_DIR"

for d in whois-enrichment geo-ip subdomain-bruteforce cert-transparency shodan-enrichment wayback-enrichment breach-check hash-identifier port-scanner social-checker url-unshortener; do
    if [ -d "$d" ]; then
        cp -r "$d" "$PLUGIN_DIR/"
        echo "[+] Installed $d"
    else
        echo "[!] Missing $d"
    fi
done

echo "[*] Installing Python dependencies..."
pip3 install -r requirements.txt
echo "[*] Done. Plugins ready at $PLUGIN_DIR"
