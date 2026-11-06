
/*
 * EKUKE OSINT WORKBENCH
 * Creator: Anonymous-beta
 * Repository: https://github.com/anonymous-beta/Ekuke
 *
 * Local-first OSINT resource directory.
 * Supports favourites, recent resources, search, JSON export,
 * external resource launching and case entity creation.
 */

const OSINT_CATEGORIES = [
  {
    name: "Social Media & Profiles",
    description: "Public social media research and profile discovery.",
    tools: [
      ["Sherlock", "Search usernames across public platforms.", "https://github.com/sherlock-project/sherlock"],
      ["WhatsMyName", "Username OSINT project.", "https://github.com/WebBreacher/WhatsMyName"],
      ["Namechk", "Username and domain availability.", "https://namechk.com/"],
      ["Social Searcher", "Search public social media mentions.", "https://www.social-searcher.com/"],
      ["Google Social Search", "Search publicly indexed social profiles.", "https://www.google.com/search?q="]
    ]
  },
  {
    name: "AI & Research",
    description: "Research assistants and public information discovery.",
    tools: [
      ["Perplexity", "Web research and cited answers.", "https://www.perplexity.ai/"],
      ["Hugging Face", "Public AI models and datasets.", "https://huggingface.co/"],
      ["Google Search", "General web research.", "https://www.google.com/search?q="],
      ["Kagi", "Independent web search.", "https://kagi.com/"]
    ]
  },
  {
    name: "Search Engines",
    description: "General and specialized search engines.",
    tools: [
      ["Google", "General web search.", "https://www.google.com/search?q="],
      ["Bing", "Microsoft web search.", "https://www.bing.com/search?q="],
      ["DuckDuckGo", "Privacy-oriented search.", "https://duckduckgo.com/?q="],
      ["Brave Search", "Independent web search.", "https://search.brave.com/search?q="],
      ["Internet Archive", "Search archived websites.", "https://web.archive.org/"]
    ]
  },
  {
    name: "Domains & DNS",
    description: "Domain registration, DNS records and certificate discovery.",
    tools: [
      ["ICANN Lookup", "Domain registration information.", "https://lookup.icann.org/"],
      ["DNSdumpster", "DNS and subdomain discovery.", "https://dnsdumpster.com/"],
      ["crt.sh", "Certificate transparency search.", "https://crt.sh/?q="],
      ["SecurityTrails", "Domain and DNS intelligence.", "https://securitytrails.com/"],
      ["DNSChecker", "Global DNS propagation checks.", "https://dnschecker.org/"],
      ["ViewDNS", "Public DNS and domain utilities.", "https://viewdns.info/"]
    ]
  },
  {
    name: "Geolocation & Maps",
    description: "Public geographic information and mapping.",
    tools: [
      ["OpenStreetMap", "Open map data.", "https://www.openstreetmap.org/"],
      ["Google Maps", "Map and location research.", "https://maps.google.com/"],
      ["GeoHack", "Coordinate-based map links.", "https://geohack.toolforge.org/"],
      ["Google Earth", "Satellite and geographic research.", "https://earth.google.com/"],
      ["Wikimapia", "Community geographic information.", "https://wikimapia.org/"]
    ]
  },
  {
    name: "Threat Intelligence",
    description: "Public security intelligence and reputation checks.",
    tools: [
      ["VirusTotal", "URL, file and hash reputation.", "https://www.virustotal.com/gui/home/search"],
      ["AbuseIPDB", "IP reputation lookup.", "https://www.abuseipdb.com/"],
      ["URLhaus", "Malicious URL intelligence.", "https://urlhaus.abuse.ch/"],
      ["AlienVault OTX", "Open threat intelligence.", "https://otx.alienvault.com/"],
      ["ThreatFox", "Public malware indicator intelligence.", "https://threatfox.abuse.ch/"],
      ["Shodan", "Internet asset search.", "https://www.shodan.io/"]
    ]
  },
  {
    name: "People Search & Public Records",
    description: "Publicly accessible records and entity research.",
    tools: [
      ["OpenCorporates", "Public company records.", "https://opencorporates.com/"],
      ["Wikidata", "Structured public knowledge graph.", "https://www.wikidata.org/"],
      ["Google Exact Name Search", "Search exact names in public pages.", "https://www.google.com/search?q="],
      ["Internet Archive", "Historical public web information.", "https://web.archive.org/"]
    ]
  },
  {
    name: "Reverse Image & Visual",
    description: "Image search and visual verification.",
    tools: [
      ["Google Lens", "Visual search.", "https://lens.google.com/"],
      ["TinEye", "Reverse image search.", "https://tineye.com/"],
      ["Bing Visual Search", "Image-based web search.", "https://www.bing.com/visualsearch"],
      ["Exif.tools", "Inspect image metadata.", "https://exif.tools/"],
      ["FotoForensics", "Image analysis and forensic research.", "https://fotoforensics.com/"]
    ]
  },
  {
    name: "Email",
    description: "Email verification and public exposure checks.",
    tools: [
      ["Have I Been Pwned", "Check an email you control for known breaches.", "https://haveibeenpwned.com/"],
      ["Hunter", "Email discovery and verification.", "https://hunter.io/"],
      ["MXToolbox", "MX and email diagnostics.", "https://mxtoolbox.com/"],
      ["MailTester", "Email configuration testing.", "https://www.mail-tester.com/"],
      ["EmailRep", "Email reputation service.", "https://emailrep.io/"]
    ]
  },
  {
    name: "Breaches & Exposure",
    description: "Personal account exposure and breach notification.",
    tools: [
      ["Have I Been Pwned", "Breach notification service.", "https://haveibeenpwned.com/"],
      ["Mozilla Monitor", "Personal email exposure monitoring.", "https://monitor.mozilla.org/"],
      ["Google Password Checkup", "Review saved account security.", "https://passwords.google.com/"],
      ["DeHashed", "Credential exposure research service.", "https://www.dehashed.com/"]
    ]
  },
  {
    name: "Companies & Organizations",
    description: "Public company and organizational intelligence.",
    tools: [
      ["OpenCorporates", "Company registry index.", "https://opencorporates.com/"],
      ["GLEIF", "Legal entity identifier records.", "https://search.gleif.org/"],
      ["Companies House", "UK company records.", "https://find-and-update.company-information.service.gov.uk/"],
      ["SEC EDGAR", "US public company filings.", "https://www.sec.gov/edgar/search/"],
      ["Open Ownership", "Public beneficial ownership data.", "https://www.openownership.org/"]
    ]
  },
  {
    name: "Code & Repositories",
    description: "Public source code and software research.",
    tools: [
      ["GitHub", "Public repository and code search.", "https://github.com/search?q="],
      ["GitLab", "Public source code projects.", "https://gitlab.com/"],
      ["grep.app", "Search public code snippets.", "https://grep.app/search?q="],
      ["Libraries.io", "Open-source package discovery.", "https://libraries.io/"],
      ["NPM", "JavaScript package discovery.", "https://www.npmjs.com/"],
      ["PyPI", "Python package discovery.", "https://pypi.org/"]
    ]
  },
  {
    name: "Metadata & Files",
    description: "File metadata and data inspection.",
    tools: [
      ["CyberChef", "Browser-based data transformation.", "https://gchq.github.io/CyberChef/"],
      ["ExifTool", "File metadata utility.", "https://exiftool.org/"],
      ["VirusTotal", "File and hash reputation checks.", "https://www.virustotal.com/gui/home/upload"],
      ["FileInfo", "File extension information.", "https://fileinfo.com/"],
      ["MD5 Online", "Hash utilities.", "https://www.md5online.org/"]
    ]
  },
  {
    name: "Telegram OSINT",
    description: "Public Telegram channels and indexed information.",
    tools: [
      ["TGStat", "Public Telegram channel analytics.", "https://tgstat.com/"],
      ["Telemetr", "Public Telegram statistics.", "https://telemetr.io/"],
      ["Telegram Search", "Search publicly indexed Telegram pages.", "https://www.google.com/search?q=site%3At.me%2F"],
      ["Telegram", "Official Telegram platform.", "https://telegram.org/"]
    ]
  },
  {
    name: "Visualization & Data",
    description: "Relationship mapping and data visualization.",
    tools: [
      ["Gephi", "Network graph visualization.", "https://gephi.org/"],
      ["RAWGraphs", "Data visualization.", "https://rawgraphs.io/"],
      ["Observable", "Interactive data notebooks.", "https://observablehq.com/"],
      ["Flourish", "Interactive data visualization.", "https://flourish.studio/"],
      ["Datawrapper", "Charts and maps.", "https://www.datawrapper.de/"]
    ]
  },
  {
    name: "Web Archives",
    description: "Historical web snapshots and archives.",
    tools: [
      ["Wayback Machine", "Archived website snapshots.", "https://web.archive.org/"],
      ["Common Crawl", "Open web crawl datasets.", "https://commoncrawl.org/"],
      ["Arquivo.pt", "Portuguese web archive.", "https://arquivo.pt/"],
      ["UK Web Archive", "UK web archive.", "https://www.webarchive.org.uk/"]
    ]
  },
  {
    name: "Phone Numbers",
    description: "Phone number validation and public lookup services.",
    tools: [
      ["Google Phone Search", "Search a number in public pages.", "https://www.google.com/search?q="],
      ["Truecaller", "Caller identification service.", "https://www.truecaller.com/"],
      ["NumVerify", "Phone number validation API.", "https://numverify.com/"],
      ["Twilio Lookup", "Phone number lookup API.", "https://www.twilio.com/docs/lookup"],
      ["PhoneInfoga", "Phone number OSINT project.", "https://github.com/sundowndev/PhoneInfoga"]
    ]
  },
  {
    name: "Transport & Vehicles",
    description: "Public transport and vehicle-related datasets.",
    tools: [
      ["OpenStreetMap", "Public road and place data.", "https://www.openstreetmap.org/"],
      ["OpenFlights", "Open flight and airport datasets.", "https://openflights.org/"],
      ["FlightAware", "Flight tracking service.", "https://www.flightaware.com/"],
      ["Flightradar24", "Flight tracking information.", "https://www.flightradar24.com/"],
      ["OpenRailwayMap", "Railway infrastructure map.", "https://www.openrailwaymap.org/"]
    ]
  },
  {
    name: "Regional / Colombia",
    description: "Colombia-focused public sources.",
    tools: [
      ["Datos Abiertos Colombia", "Colombian open government data.", "https://www.datos.gov.co/"],
      ["RUES", "Colombian business registry.", "https://www.rues.org.co/"],
      ["Google Colombia Search", "Search public Colombian sources.", "https://www.google.com/search?q="]
    ]
  },
  {
    name: "Web Traffic & SEO",
    description: "Website traffic and technology profiling.",
    tools: [
      ["Similarweb", "Website traffic intelligence.", "https://www.similarweb.com/"],
      ["Google Trends", "Search interest trends.", "https://trends.google.com/trends/"],
      ["BuiltWith", "Website technology profiling.", "https://builtwith.com/"],
      ["Wappalyzer", "Website technology detection.", "https://www.wappalyzer.com/"],
      ["Ahrefs", "SEO and backlink research.", "https://ahrefs.com/"]
    ]
  },
  {
    name: "Usernames",
    description: "Public username and handle discovery.",
    tools: [
      ["Sherlock", "Username search project.", "https://github.com/sherlock-project/sherlock"],
      ["WhatsMyName", "Username discovery project.", "https://github.com/WebBreacher/WhatsMyName"],
      ["Namechk", "Username availability.", "https://namechk.com/"],
      ["KnowEm", "Username and brand search.", "https://knowem.com/"]
    ]
  },
  {
    name: "Documents & Public Data",
    description: "Public documents and open datasets.",
    tools: [
      ["Google Filetype Search", "Search publicly indexed documents.", "https://www.google.com/search?q="],
      ["Kaggle", "Public datasets and notebooks.", "https://www.kaggle.com/datasets"],
      ["Data.gov", "US government open data.", "https://data.gov/"],
      ["World Bank Data", "Global development datasets.", "https://data.worldbank.org/"],
      ["UN Data", "United Nations datasets.", "https://data.un.org/"]
    ]
  },
  {
    name: "Fact-check & Verification",
    description: "Verify claims, media and source context.",
    tools: [
      ["Google Fact Check Explorer", "Search fact checks.", "https://toolbox.google.com/factcheck/explorer"],
      ["Snopes", "Fact-checking archive.", "https://www.snopes.com/"],
      ["InVID-WeVerify", "Image and video verification toolkit.", "https://www.invid-project.eu/tools-and-services/invid-verification-plugin/"],
      ["Full Fact", "Independent fact checking.", "https://fullfact.org/"],
      ["Reuters Fact Check", "Reuters verification reports.", "https://www.reuters.com/fact-check/"]
    ]
  },
  {
    name: "APIs & Developer Services",
    description: "Official API documentation and integrations.",
    tools: [
      ["GitHub REST API", "Official GitHub API documentation.", "https://docs.github.com/en/rest"],
      ["HIBP API", "Have I Been Pwned API documentation.", "https://haveibeenpwned.com/API/v3"],
      ["Shodan API", "Shodan API documentation.", "https://developer.shodan.io/api"],
      ["OpenCorporates API", "Company data API documentation.", "https://api.opencorporates.com/documentation/API-Reference"],
      ["IPinfo", "IP intelligence API.", "https://ipinfo.io/developers"],
      ["Nominatim", "OpenStreetMap geocoding API.", "https://nominatim.org/release-docs/latest/api/Overview/"]
    ]
  },
  {
    name: "Tor & Onion Research",
    description: "Tor documentation and onion service research.",
    tools: [
      ["Tor Project", "Official Tor documentation.", "https://www.torproject.org/"],
      ["Ahmia", "Onion service search.", "https://ahmia.fi/"],
      ["Tor Onion Services", "Official onion service documentation.", "https://community.torproject.org/onion-services/"]
    ]
  }
];

const OSINT = {
  favorites: "ekuke_osint_favorites",
  recent: "ekuke_osint_recent"
};

function osintRead(key) {
  try {
    return JSON.parse(localStorage.getItem(key) || "[]");
  } catch {
    return [];
  }
}

function osintWrite(key, value) {
  localStorage.setItem(key, JSON.stringify(value));
}

function osintEscape(value) {
  return String(value ?? "").replace(/[&<>"']/g, char => ({
    "&": "&amp;",
    "<": "&lt;",
    ">": "&gt;",
    '"': "&quot;",
    "'": "&#39;"
  })[char]);
}

function osintSlug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-");
}

const OSINT_TOOLS = OSINT_CATEGORIES.flatMap(category =>
  category.tools.map(tool => ({
    id: osintSlug(category.name + "-" + tool[0]),
    category: category.name,
    name: tool[0],
    description: tool[1],
    url: tool[2],
    isSearch: tool[2].endsWith("q=")
  }))
);

function osintFiltered() {
  const query = document.getElementById("osint-search")?.value
    .toLowerCase().trim() || "";

  const category = document.getElementById("osint-category")?.value || "";
  const mode = document.getElementById("osint-mode")?.value || "all";

  const favorites = osintRead(OSINT.favorites);
  const recent = osintRead(OSINT.recent);

  return OSINT_TOOLS.filter(tool => {
    const matchesCategory = !category || tool.category === category;
    const matchesSearch = !query ||
      (tool.name + " " + tool.description + " " + tool.category)
        .toLowerCase().includes(query);

    const matchesMode =
      mode === "all" ||
      (mode === "favorites" && favorites.includes(tool.id)) ||
      (mode === "recent" && recent.includes(tool.id));

    return matchesCategory && matchesSearch && matchesMode;
  }).sort((a, b) => {
    if (mode !== "recent") return a.name.localeCompare(b.name);
    return recent.indexOf(a.id) - recent.indexOf(b.id);
  });
}

function osintToggleFavorite(id) {
  let favorites = osintRead(OSINT.favorites);

  if (favorites.includes(id)) {
    favorites = favorites.filter(item => item !== id);
  } else {
    favorites.unshift(id);
  }

  osintWrite(OSINT.favorites, favorites);
  osintRender();
}

function osintLaunch(id) {
  const tool = OSINT_TOOLS.find(item => item.id === id);
  if (!tool) return;

  let url = tool.url;

  if (tool.isSearch) {
    const query = document.getElementById("osint-search").value.trim();

    if (!query) {
      alert("Enter a search term first.");
      document.getElementById("osint-search").focus();
      return;
    }

    url += encodeURIComponent(query);
  }

  const recent = osintRead(OSINT.recent).filter(item => item !== id);
  recent.unshift(id);
  osintWrite(OSINT.recent, recent.slice(0, 50));

  window.open(url, "_blank", "noopener,noreferrer");
  osintRender();
}

async function osintAddToCase(id) {
  const tool = OSINT_TOOLS.find(item => item.id === id);
  if (!tool) return;

  const bridge = window.__TAURI__;

  if (!bridge?.core?.invoke) {
    alert("EKUKE backend is not available.");
    return;
  }

  const label = prompt(
    "Enter the finding or entity label to add to your current case:"
  );

  if (!label || !label.trim()) return;

  const type = prompt(
    "Choose entity type:\nDomain, Email, IPv4, Phone, Handle, Person, Organization, URL, Document, Other",
    "Other"
  );

  if (!type) return;

  try {
    await bridge.core.invoke("create_entity", {
      entityType: type.trim(),
      label: label.trim(),
      properties: {
        source: "EKUKE OSINT Workbench",
        resource: tool.name,
        resource_category: tool.category,
        resource_url: tool.url,
        recorded_at: new Date().toISOString()
      }
    });

    alert("Finding added to the current EKUKE case.");

    document.dispatchEvent(new CustomEvent("ekuke:osint-entity-added"));
  } catch (error) {
    alert("Could not add finding:\n" + error);
  }
}

function osintExport() {
  const payload = {
    application: "EKUKE",
    module: "OSINT Workbench",
    author: "Anonymous-beta",
    exported_at: new Date().toISOString(),
    favorites: osintRead(OSINT.favorites),
    recently_opened: osintRead(OSINT.recent),
    categories: OSINT_CATEGORIES,
    resources: OSINT_TOOLS
  };

  const blob = new Blob(
    [JSON.stringify(payload, null, 2)],
    { type: "application/json" }
  );

  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");

  anchor.href = url;
  anchor.download = "ekuke-osint-directory.json";
  anchor.click();

  URL.revokeObjectURL(url);
}

function osintRender() {
  const container = document.getElementById("osint-results");
  if (!container) return;

  const tools = osintFiltered();
  const favorites = osintRead(OSINT.favorites);

  document.getElementById("osint-count").textContent =
    tools.length + " resources";

  container.innerHTML = tools.length
    ? tools.map(tool => `
      <article class="osint-card">
        <div class="osint-card-heading">
          <span class="osint-category-tag">
            ${osintEscape(tool.category)}
          </span>

          <button
            class="osint-star ${favorites.includes(tool.id) ? "active" : ""}"
            data-osint-favorite="${tool.id}"
            title="Toggle favourite"
          >
            ${favorites.includes(tool.id) ? "★" : "☆"}
          </button>
        </div>

        <h3>${osintEscape(tool.name)}</h3>

        <p>${osintEscape(tool.description)}</p>

        <div class="osint-card-actions">
          <button
            class="osint-open"
            data-osint-launch="${tool.id}"
          >
            Open ↗
          </button>

          <button
            class="osint-add"
            data-osint-add="${tool.id}"
          >
            Add finding
          </button>
        </div>
      </article>
    `).join("")
    : `<div class="osint-empty">
        No matching resources found.
      </div>`;
}

function osintBuildCategories() {
  const select = document.getElementById("osint-category");

  select.innerHTML =
    `<option value="">All categories</option>` +
    OSINT_CATEGORIES.map(category =>
      `<option value="${osintEscape(category.name)}">
        ${osintEscape(category.name)}
      </option>`
    ).join("");
}

function osintOpen() {
  document.getElementById("osint-overlay").classList.remove("hidden");
  osintRender();
}

function osintClose() {
  document.getElementById("osint-overlay").classList.add("hidden");
}

function osintInitialize() {
  if (document.getElementById("btn-osint-workbench")) return;

  const toolbar = document.getElementById("toolbar");
  if (!toolbar) return;

  const button = document.createElement("button");
  button.id = "btn-osint-workbench";
  button.className = "btn";
  button.textContent = "◎ OSINT Workbench";
  button.addEventListener("click", osintOpen);

  toolbar.insertBefore(
    button,
    document.getElementById("case-name")
  );

  osintBuildCategories();

  document.getElementById("osint-close").addEventListener("click", osintClose);

  document.getElementById("osint-search").addEventListener("input", osintRender);

  document.getElementById("osint-category").addEventListener("change", osintRender);

  document.getElementById("osint-mode").addEventListener("change", osintRender);

  document.getElementById("osint-export").addEventListener("click", osintExport);

  document.getElementById("osint-results").addEventListener("click", event => {
    const favorite = event.target.closest("[data-osint-favorite]");
    const launch = event.target.closest("[data-osint-launch]");
    const add = event.target.closest("[data-osint-add]");

    if (favorite) {
      osintToggleFavorite(favorite.dataset.osintFavorite);
    }

    if (launch) {
      osintLaunch(launch.dataset.osintLaunch);
    }

    if (add) {
      osintAddToCase(add.dataset.osintAdd);
    }
  });

  document.getElementById("osint-random").addEventListener("click", () => {
    const items = osintFiltered();
    if (!items.length) return;

    const selected = items[Math.floor(Math.random() * items.length)];

    document.getElementById("osint-search").value = "";
    document.getElementById("osint-category").value = selected.category;

    osintRender();
  });

  document.addEventListener("keydown", event => {
    if (event.key === "Escape") osintClose();
  });

  osintRender();
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", osintInitialize);
} else {
  osintInitialize();
}
