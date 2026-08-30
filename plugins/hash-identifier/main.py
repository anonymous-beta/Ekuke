#!/usr/bin/env python3
import json, sys, re

HASH_PATTERNS = [
    (r"^[a-f0-9]{32}$", "MD5", 100),
    (r"^[a-f0-9]{32}:[a-zA-Z0-9]+$", "MD5 (pass:salt)", 100),
    (r"^[a-f0-9]{40}$", "SHA-1", 100),
    (r"^[a-f0-9]{64}$", "SHA-256", 100),
    (r"^[a-f0-9]{128}$", "SHA-512", 100),
    (r"^[a-f0-9]{56}$", "SHA-224", 100),
    (r"^[a-f0-9]{96}$", "SHA-384", 100),
    (r"^\$1\$[a-zA-Z0-9./]{8}\$[a-zA-Z0-9./]{22}$", "MD5 Crypt", 100),
    (r"^\$2[aby]?\$[0-9]{2}\$[./A-Za-z0-9]{53}$", "Bcrypt", 100),
    (r"^\$5\$[a-zA-Z0-9./]{16}\$[a-zA-Z0-9./]{43}$", "SHA-256 Crypt", 100),
    (r"^\$6\$[a-zA-Z0-9./]{16}\$[a-zA-Z0-9./]{86}$", "SHA-512 Crypt", 100),
    (r"^\$argon2i\$", "Argon2i", 100),
    (r"^\$argon2d\$", "Argon2d", 100),
    (r"^\$argon2id\$", "Argon2id", 100),
    (r"^\$pbkdf2-sha1\$", "PBKDF2-SHA1", 100),
    (r"^\$pbkdf2-sha256\$", "PBKDF2-SHA256", 100),
    (r"^\$pbkdf2-sha512\$", "PBKDF2-SHA512", 100),
    (r"^[a-f0-9]{16}$", "MySQL3.x / MySQL4.x", 100),
    (r"^\*[a-f0-9]{40}$", "MySQL5.x", 100),
    (r"^[a-f0-9]{34}$", "NTLMv2", 90),
    (r"^0x0100[a-f0-9]{8}[a-f0-9]{40}$", "MSSQL(2000)", 100),
    (r"^0x0100[a-f0-9]{8}[a-f0-9]{80}$", "MSSQL(2005)", 100),
    (r"^0x0200[a-f0-9]{8}[a-f0-9]{128}$", "MSSQL(2012+)", 100),
    (r"^[A-F0-9]{32}$", "LM / NTLM", 90),
    (r"^\$apr1\$[a-zA-Z0-9./]{8}\$[a-zA-Z0-9./]{22}$", "Apache MD5", 100),
    (r"^\$H\$[a-zA-Z0-9./]{31}$", "phpBB3", 100),
    (r"^\$P\$[a-zA-Z0-9./]{31}$", "WordPress MD5", 100),
    (r"^sha1\$[a-zA-Z0-9]+\$[a-f0-9]{40}$", "SHA1(Django)", 100),
    (r"^pbkdf2_sha256\$[0-9]+\$[a-zA-Z0-9]+\$[a-zA-Z0-9/+=]{44}$", "PBKDF2(Django)", 100),
    (r"^[a-f0-9]{65}$", "Drupal > v7.x", 100),
    (r"^[a-f0-9]{30}$", "MD5(Unix)", 80),
    (r"^[a-f0-9]{8}$", "Adler-32 / CRC-32", 70),
    (r"^[a-zA-Z0-9]{13}$", "DES(Unix) / BigCrypt", 80),
    (r"^crypt16\$", "BigCrypt", 80),
    (r"^\$md5\$", "Sun MD5", 80),
]

def main():
    data = json.load(sys.stdin)
    hash_str = data["entity"]["label"].strip()

    matches = []
    for pattern, name, confidence in HASH_PATTERNS:
        if re.match(pattern, hash_str, re.IGNORECASE):
            matches.append({"type": name, "confidence": confidence})

    if not matches:
        print(json.dumps({"error": "Unknown hash format"}))
        return

    entities = []
    relationships = []
    for match in matches:
        entities.append({
            "entity_type": "Hash_Type",
            "label": match["type"],
            "properties": {
                "confidence": match["confidence"],
                "original_hash": hash_str,
                "hash_length": len(hash_str),
                "source": "hash-identifier"
            }
        })
        relationships.append({
            "rel_type": "IDENTIFIED_AS",
            "source_label": hash_str,
            "source_type": "Other",
            "target_label": match["type"],
            "target_type": "Hash_Type",
            "properties": {"confidence": match["confidence"]}
        })

    print(json.dumps({"entities": entities, "relationships": relationships}))

if __name__ == "__main__":
    main()
