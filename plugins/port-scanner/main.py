#!/usr/bin/env python3
import json, sys, socket, asyncio

TOP_PORTS = [21,22,23,25,53,80,110,111,135,139,143,443,445,993,995,1723,3306,3389,5900,8080,8443,9200,9300,10000]

async def check_port(ip, port, timeout):
    try:
        reader, writer = await asyncio.wait_for(
            asyncio.open_connection(ip, port),
            timeout=timeout
        )
        writer.close()
        await writer.wait_closed()
        return port
    except:
        return None

async def scan(ip, ports, sem_count, timeout):
    sem = asyncio.Semaphore(sem_count)
    async def task(port):
        async with sem:
            return await check_port(ip, port, timeout)
    results = await asyncio.gather(*[task(p) for p in ports])
    return [r for r in results if r is not None]

def main():
    data = json.load(sys.stdin)
    ip = data["entity"]["label"]
    config = data.get("config", {})
    ports_str = config.get("ports", "")
    threads = int(config.get("threads", "200"))
    timeout = float(config.get("timeout", "2.0"))

    if ports_str:
        ports = []
        for part in ports_str.split(","):
            part = part.strip()
            if "-" in part:
                start, end = part.split("-")
                ports.extend(range(int(start), int(end)+1))
            else:
                ports.append(int(part))
    else:
        ports = TOP_PORTS

    try:
        open_ports = asyncio.run(scan(ip, ports, threads, timeout))
        entities = []
        relationships = []

        for port in sorted(open_ports):
            port_label = f"{ip}:{port}"
            try:
                service = socket.getservbyport(port, "tcp")
            except:
                service = "unknown"

            entities.append({
                "entity_type": "Port",
                "label": port_label,
                "properties": {
                    "port": port,
                    "protocol": "tcp",
                    "service": service,
                    "source": "port-scanner"
                }
            })
            relationships.append({
                "rel_type": "HAS_OPEN_PORT",
                "source_label": ip,
                "source_type": "IPv4",
                "target_label": port_label,
                "target_type": "Port",
                "properties": {"service": service}
            })

        print(json.dumps({"entities": entities, "relationships": relationships}))
    except Exception as e:
        print(json.dumps({"error": str(e)}))

if __name__ == "__main__":
    main()
