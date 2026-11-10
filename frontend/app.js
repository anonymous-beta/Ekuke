Path: "frontend/app.js"

/* =========================================================
   EKUKE — Investigation Frontend
   Creator: Anonymous-beta
   Version: 0.3.0
   Architecture: Tauri 2 + SQLite + Cytoscape
========================================================= */

const TAURI = window.__TAURI__;
const invoke = TAURI?.core?.invoke;

let cy = null;
let currentCase = null;
let activeEntity = null;
let aiHistory = [];
let settingsConfig = null;
let graphLoading = false;
let linkSearchTimer = null;
let searchTimer = null;

const $ = id => document.getElementById(id);

const COLORS = {
    Domain: "#3b82f6",
    Email: "#10b981",
    IPv4: "#f59e0b",
    Phone: "#8b5cf6",
    Handle: "#ec4899",
    Person: "#ef4444",
    Organization: "#06b6d4",
    URL: "#84cc16",
    Document: "#f97316",
    Other: "#8b929a"
};

function escapeHtml(value) {
    return String(value ?? "").replace(/[&<>"']/g, c => ({
        "&": "&amp;",
        "<": "&lt;",
        ">": "&gt;",
        '"': "&quot;",
        "'": "&#39;"
    })[c]);
}

function setStatus(message, error = false) {
    const el = $("status-msg");
    if (!el) return;

    el.textContent = message;
    el.style.color = error ? "#ff7777" : "#9bdc91";
}

function requireCase() {
    if (!currentCase) {
        throw new Error("Create or open an investigation case first.");
    }
}

async function safeInvoke(command, args = {}) {
    if (typeof invoke !== "function") {
        throw new Error("Tauri backend is unavailable.");
    }

    return invoke(command, args);
}

function showError(error) {
    const message = error?.message || String(error);
    console.error(error);
    setStatus(message, true);
    alert(message);
}

function setBusy(button, busy, label) {
    if (!button) return;

    button.disabled = busy;

    if (label) {
        if (busy) {
            button.dataset.originalText = button.textContent;
            button.textContent = label;
        } else {
            button.textContent =
                button.dataset.originalText || button.textContent;
        }
    }
}

/* =========================================================
   INITIALIZATION
========================================================= */

async function init() {
    if (typeof invoke !== "function") {
        setStatus("Tauri bridge unavailable. Rebuild the desktop application.", true);
        return;
    }

    bindMainButtons();
    bindSettingsTabs();
    bindKeyboardShortcuts();
    initializeGraph();

    try {
        await loadConfig();
    } catch (e) {
        console.error("Configuration:", e);
    }

    try {
        await loadPlugins();
    } catch (e) {
        console.error("Plugins:", e);
    }

    try {
        currentCase = await safeInvoke("get_current_case");

        if (currentCase) {
            updateCaseLabel();
            await loadGraph();
        }
    } catch (e) {
        console.error("Case initialization:", e);
    }

    try {
        const version = await safeInvoke("get_app_version");
        setStatus(`EKUKE ${version} · Backend connected`);
    } catch (e) {
        setStatus("Backend connection failed: " + e, true);
    }
}

function bindMainButtons() {
    const bindings = {
        "btn-new-case": showNewCaseModal,
        "btn-open-case": showOpenCaseModal,
        "btn-export-case": exportCase,
        "btn-import-case": importCase,
        "btn-delete-case": deleteCurrentCase,
        "btn-add-entity": showAddEntityModal,
        "btn-extract": showExtractModal,
        "btn-scrape": showScrapeModal,
        "btn-export-png": exportPng,
        "btn-settings": showSettings,
        "btn-ai-chat": toggleAiPanel,
        "btn-ai-close": toggleAiPanel,
        "btn-ai-send": sendAiMessage,
        "btn-ai-summary": summarizeCase,
        "btn-close-detail": closeDetail,
        "modal-cancel": closeModal,
        "settings-cancel": closeSettings,
        "settings-save": saveSettings,
        "btn-ai-test": testAiConnection,
        "btn-add-key": saveExtraKey
    };

    for (const [id, handler] of Object.entries(bindings)) {
        $(id)?.addEventListener("click", () => {
            Promise.resolve(handler()).catch(showError);
        });
    }

    $("ai-input")?.addEventListener("keydown", e => {
        if (e.key === "Enter") sendAiMessage().catch(showError);
    });

    $("search-bar")?.addEventListener("input", e => {
        clearTimeout(searchTimer);
        searchTimer = setTimeout(
            () => handleSearch(e.target.value).catch(showError),
            250
        );
    });

    $("modal-overlay")?.addEventListener("click", e => {
        if (e.target.id === "modal-overlay") closeModal();
    });

    $("settings-overlay")?.addEventListener("click", e => {
        if (e.target.id === "settings-overlay") closeSettings();
    });
}

function bindSettingsTabs() {
    document.querySelectorAll(".tab").forEach(tab => {
        tab.addEventListener("click", () => {
            document.querySelectorAll(".tab").forEach(t =>
                t.classList.remove("active")
            );

            document.querySelectorAll(".tab-body").forEach(body =>
                body.classList.add("hidden")
            );

            tab.classList.add("active");
            $(tab.dataset.tab)?.classList.remove("hidden");
        });
    });
}

function bindKeyboardShortcuts() {
    document.addEventListener("keydown", e => {
        if (e.key === "Escape") {
            closeModal();
            closeSettings();
        }

        if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
            e.preventDefault();
            $("search-bar")?.focus();
        }
    });
}

/* =========================================================
   GRAPH ENGINE
========================================================= */

function initializeGraph() {
    if (window.__cyFailed || typeof window.cytoscape !== "function") {
        setStatus("Cytoscape could not load. Check the graph library.", true);
        return;
    }

    cy = window.cytoscape({
        container: $("graph-container"),

        style: [
            {
                selector: "node",
                style: {
                    "background-color": "data(color)",
                    "label": "data(label)",
                    "color": "#e4e7e9",
                    "font-size": 11,
                    "text-valign": "bottom",
                    "text-margin-y": 7,
                    "text-outline-width": 2,
                    "text-outline-color": "#101316",
                    "width": 44,
                    "height": 44,
                    "border-width": 2,
                    "border-color": "#59636b"
                }
            },
            {
                selector: "edge",
                style: {
                    "width": 2,
                    "line-color": "#64717a",
                    "target-arrow-color": "#64717a",
                    "target-arrow-shape": "triangle",
                    "curve-style": "bezier",
                    "label": "data(label)",
                    "font-size": 9,
                    "color": "#c0c6ca",
                    "text-background-color": "#101316",
                    "text-background-opacity": 0.9,
                    "text-background-padding": 3,
                    "text-rotation": "autorotate"
                }
            },
            {
                selector: "node:selected",
                style: {
                    "border-color": "#e6a15a",
                    "border-width": 4
                }
            },
            {
                selector: "edge:selected",
                style: {
                    "line-color": "#e6a15a",
                    "target-arrow-color": "#e6a15a",
                    "width": 4
                }
            },
            {
                selector: ".faded",
                style: {
                    "opacity": 0.14
                }
            },
            {
                selector: ".highlighted",
                style: {
                    "opacity": 1,
                    "border-width": 4
                }
            }
        ],

        layout: {
            name: "cose",
            animate: false,
            padding: 45
        },

        minZoom: 0.15,
        maxZoom: 3,
        wheelSensitivity: 0.2
    });

    cy.on("tap", "node", event => {
        showEntityDetail(event.target.id()).catch(showError);
    });

    cy.on("tap", "edge", event => {
        showRelationshipDetail(event.target.id()).catch(showError);
    });

    cy.on("cxttap", "node", event => {
        const nodeId = event.target.id();
        if (confirm("Delete this entity and its relationships?")) {
            deleteEntity(nodeId).catch(showError);
        }
    });

    cy.on("cxttap", "edge", event => {
        const edgeId = event.target.id();

        if (confirm("Delete this relationship?")) {
            deleteRelationship(edgeId).catch(showError);
        }
    });
}

async function loadGraph() {
    if (!currentCase || !cy || graphLoading) return;

    graphLoading = true;

    try {
        const [entities, relationships] = await Promise.all([
            safeInvoke("get_entities", {}),
            safeInvoke("get_relationships", {})
        ]);

        const entityIds = new Set(entities.map(e => e.id));

        cy.elements().remove();

        const nodes = entities.map(entity => ({
            group: "nodes",
            data: {
                id: entity.id,
                label: entity.label,
                type: entity.entity_type,
                color: COLORS[entity.entity_type] || COLORS.Other,
                properties: entity.properties || {}
            }
        }));

        const edges = relationships
            .filter(rel =>
                entityIds.has(rel.source_id) &&
                entityIds.has(rel.target_id)
            )
            .map(rel => ({
                group: "edges",
                data: {
                    id: rel.id,
                    source: rel.source_id,
                    target: rel.target_id,
                    label: rel.rel_type,
                    rel_type: rel.rel_type,
                    properties: rel.properties || {}
                }
            }));

        cy.add([...nodes, ...edges]);

        cy.layout({
            name: "cose",
            animate: true,
            animationDuration: 350,
            nodeRepulsion: 6500,
            idealEdgeLength: 130,
            padding: 45
        }).run();

        setStatus(
            `${entities.length} entities · ${edges.length} relationships`
        );
    } finally {
        graphLoading = false;
    }
}

function changeGraphLayout(name) {
    if (!cy) return;

    const supported = [
        "cose",
        "circle",
        "concentric",
        "breadthfirst",
        "grid"
    ];

    if (!supported.includes(name)) {
        throw new Error("Unsupported graph layout.");
    }

    cy.layout({
        name,
        animate: true,
        padding: 40
    }).run();
}

/* =========================================================
   CASE MANAGEMENT
========================================================= */

function updateCaseLabel() {
    const label = $("case-name");

    if (label) {
        label.textContent = currentCase
            ? `CASE: ${currentCase.name}`
            : "NO ACTIVE CASE";
    }
}

function showNewCaseModal() {
    showModal(
        "Create Investigation",
        `
        <div class="detail-row">
            <label>Investigation name</label>
            <input id="case-name-input"
                   type="text"
                   placeholder="Investigation name"
                   maxlength="120">
        </div>
        `,
        async () => {
            const name = $("case-name-input")?.value.trim();

            if (!name) {
                throw new Error("Investigation name is required.");
            }

            currentCase = await safeInvoke("create_case", { name });

            updateCaseLabel();
            closeModal();
            await loadGraph();
        }
    );

    setTimeout(() => $("case-name-input")?.focus(), 50);
}

async function showOpenCaseModal() {
    const cases = await safeInvoke("list_cases");

    const html = cases.length
        ? cases.map(c => `
            <button type="button"
                    class="entity-list-item case-select"
                    data-case-id="${escapeHtml(c.id)}">
                <span class="type-badge">
                    ${escapeHtml(c.status)}
                </span>
                ${escapeHtml(c.name)}
            </button>
        `).join("")
        : `<p class="muted">No saved investigations found.</p>`;

    showModal("Open Investigation", html, async () => {});

    $("modal-confirm")?.classList.add("hidden");

    document.querySelectorAll(".case-select").forEach(button => {
        button.addEventListener("click", async () => {
            currentCase = await safeInvoke("open_case", {
                caseId: button.dataset.caseId
            });

            aiHistory = [];
            updateCaseLabel();
            closeModal();
            await loadGraph();
        });
    });
}

async function deleteCurrentCase() {
    requireCase();

    if (!confirm(
        `Permanently delete "${currentCase.name}" and all its case data?`
    )) return;

    await safeInvoke("delete_case", {
        caseId: currentCase.id
    });

    currentCase = null;
    activeEntity = null;
    aiHistory = [];

    if (cy) cy.elements().remove();

    closeDetail();
    updateCaseLabel();
    setStatus("Investigation deleted.");
}

async function exportCase() {
    requireCase();

    const path = await safeInvoke("export_case", {
        caseId: currentCase.id
    });

    alert("Investigation exported:\n" + path);
}

async function importCase() {
    const dialog = TAURI?.dialog;

    if (!dialog?.open) {
        throw new Error("File picker is unavailable.");
    }

    const path = await dialog.open({
        multiple: false,
        filters: [{
            name: "EKUKE Investigation",
            extensions: ["ekuke"]
        }]
    });

    if (!path) return;

    currentCase = await safeInvoke("import_case", { path });

    updateCaseLabel();
    await loadGraph();

    alert("Investigation imported successfully.");
}

/* =========================================================
   ENTITY MANAGEMENT
========================================================= */

const ENTITY_TYPES = [
    "Domain",
    "Email",
    "IPv4",
    "Phone",
    "Handle",
    "Person",
    "Organization",
    "URL",
    "Document",
    "Other"
];

function showAddEntityModal() {
    requireCase();

    showModal(
        "Add Entity",
        `
        <div class="detail-row">
            <label>Entity type</label>
            <select id="new-entity-type">
                ${ENTITY_TYPES.map(type =>
                    `<option value="${type}">${type}</option>`
                ).join("")}
            </select>
        </div>

        <div class="detail-row">
            <label>Entity value</label>
            <input id="new-entity-label"
                   type="text"
                   placeholder="example.com">
        </div>
        `,
        async () => {
            const entityType = $("new-entity-type").value;
            const label = $("new-entity-label").value.trim();

            if (!label) {
                throw new Error("Entity value is required.");
            }

            await safeInvoke("create_entity", {
                entityType,
                label,
                properties: null
            });

            closeModal();
            await loadGraph();
        }
    );
}

async function deleteEntity(id) {
    await safeInvoke("delete_entity", { id });

    closeDetail();
    await loadGraph();

    setStatus("Entity and connected relationships deleted.");
}

async function showEntityDetail(id) {
    requireCase();

    const entity = await safeInvoke("get_entity_by_id", { id });

    activeEntity = entity;

    $("detail-panel")?.classList.remove("hidden");

    const properties = Object.entries(entity.properties || {})
        .map(([key, value]) => `
            <div class="detail-row">
                <label>${escapeHtml(key)}</label>
                <input type="text"
                       data-prop="${escapeHtml(key)}"
                       value="${escapeHtml(
                           typeof value === "string"
                               ? value
                               : JSON.stringify(value)
                       )}">
            </div>
        `).join("");

    $("detail-content").innerHTML = `
        <div class="detail-row">
            <label>Entity ID</label>
            <input value="${escapeHtml(entity.id)}" readonly>
        </div>

        <div class="detail-row">
            <label>Type</label>
            <input value="${escapeHtml(entity.entity_type)}" readonly>
        </div>

        <div class="detail-row">
            <label>Label</label>
            <input id="detail-label"
                   value="${escapeHtml(entity.label)}">
        </div>

        ${properties}

        <hr>

        <h4>CREATE RELATIONSHIP</h4>

        <div class="detail-row">
            <label>Find target entity</label>
            <input id="link-target"
                   type="search"
                   placeholder="Search existing entities">
        </div>

        <div class="detail-row">
            <label>Relationship type</label>
            <input id="link-type"
                   placeholder="owns, resolves_to, mentions...">
        </div>

        <div class="detail-row">
            <label>Direction</label>
            <p class="muted">
                ${escapeHtml(entity.label)} → selected target
            </p>
        </div>

        <div id="link-results">
            <p class="muted">
                Search and select the exact target entity.
            </p>
        </div>

        <hr>

        <button class="btn primary" id="btn-update-entity">
            Save Entity
        </button>

        <button class="btn" id="btn-enrich-entity">
            Enrich Entity
        </button>

        <button class="btn" id="btn-run-transform">
            Run Plugin
        </button>

        <button class="btn" id="btn-neighborhood">
            View Neighbourhood
        </button>

        <button class="btn danger" id="btn-delete-entity">
            Delete Entity
        </button>
    `;

    $("link-target").addEventListener("input", () => {
        clearTimeout(linkSearchTimer);

        linkSearchTimer = setTimeout(() => {
            searchLinkTargets(entity, $("link-target").value)
                .catch(showError);
        }, 250);
    });

    $("btn-update-entity").addEventListener("click", () => {
        updateEntity(entity).catch(showError);
    });

    $("btn-delete-entity").addEventListener("click", () => {
        if (confirm("Delete this entity and its relationships?")) {
            deleteEntity(entity.id).catch(showError);
        }
    });

    $("btn-enrich-entity").addEventListener("click", () => {
        enrichEntity(entity.id).catch(showError);
    });

    $("btn-run-transform").addEventListener("click", () => {
        showTransformModal(entity).catch(showError);
    });

    $("btn-neighborhood").addEventListener("click", () => {
        showNeighborhood(entity.id).catch(showError);
    });
}

async function updateEntity(entity) {
    const label = $("detail-label").value.trim();

    if (!label) {
        throw new Error("Entity label cannot be empty.");
    }

    const properties = {};

    document.querySelectorAll("#detail-content [data-prop]")
        .forEach(input => {
            properties[input.dataset.prop] = input.value;
        });

    await safeInvoke("update_entity", {
        id: entity.id,
        label,
        properties
    });

    await loadGraph();
    await showEntityDetail(entity.id);

    setStatus("Entity updated.");
}

async function enrichEntity(id) {
    const button = $("btn-enrich-entity");

    setBusy(button, true, "Enriching...");

    try {
        const result = await safeInvoke("enrich_entity", { id });

        await loadGraph();
        await showEntityDetail(id);

        alert(
            "Enrichment completed\n\n" +
            `Properties: ${result.properties_added}\n` +
            `Entities: ${result.entities_created}\n` +
            `Relationships: ${result.relationships_created}\n\n` +
            (result.notes || []).join("\n")
        );
    } finally {
        setBusy($("btn-enrich-entity"), false);
    }
}

/* =========================================================
   GRAPH LINKING — EXACT TARGET SELECTION
========================================================= */

async function searchLinkTargets(source, query) {
    const box = $("link-results");
    if (!box) return;

    const q = query.trim();

    if (q.length < 2) {
        box.innerHTML =
            `<p class="muted">Enter at least two characters.</p>`;
        return;
    }

    box.innerHTML = `<p class="muted">Searching case entities...</p>`;

    const results = await safeInvoke("search_entities", {
        query: q,
        limit: 100
    });

    const filtered = results.filter(entity =>
        entity.id !== source.id
    );

    if (!filtered.length) {
        box.innerHTML =
            `<p class="muted">No matching target entities found.</p>`;
        return;
    }

    box.innerHTML = "";

    filtered.forEach(entity => {
        const button = document.createElement("button");

        button.type = "button";
        button.className = "btn link-result";
        button.style.width = "100%";
        button.style.textAlign = "left";
        button.style.marginBottom = "6px";

        button.textContent =
            `${entity.entity_type} · ${entity.label} · ${entity.id.slice(0, 8)}`;

        button.addEventListener("click", async () => {
            const relType = $("link-type").value.trim();

            if (!relType) {
                alert("Enter a relationship type first.");
                $("link-type").focus();
                return;
            }

            if (entity.id === source.id) {
                alert("Self-links are not allowed.");
                return;
            }

            const alreadyExists = cy?.edges().some(edge =>
                edge.data("source") === source.id &&
                edge.data("target") === entity.id &&
                edge.data("rel_type") === relType
            );

            if (alreadyExists) {
                alert("This relationship already exists in the graph.");
                return;
            }

            button.disabled = true;
            button.textContent = "Creating relationship...";

            try {
                await safeInvoke("create_relationship", {
                    sourceId: source.id,
                    targetId: entity.id,
                    relType,
                    properties: null
                });

                await loadGraph();
                await showEntityDetail(source.id);

                setStatus(
                    `Relationship created: ${source.label} → ${entity.label}`
                );
            } catch (error) {
                button.disabled = false;
                button.textContent =
                    `${entity.entity_type} · ${entity.label} · ${entity.id.slice(0, 8)}`;

                throw error;
            }
        });

        box.appendChild(button);
    });
}

async function showRelationshipDetail(id) {
    const edge = cy?.getElementById(id);

    if (!edge || edge.empty()) return;

    const sourceId = edge.data("source");
    const targetId = edge.data("target");

    const [source, target] = await Promise.all([
        safeInvoke("get_entity_by_id", { id: sourceId }),
        safeInvoke("get_entity_by_id", { id: targetId })
    ]);

    $("detail-panel")?.classList.remove("hidden");

    $("detail-content").innerHTML = `
        <h4>RELATIONSHIP</h4>

        <div class="detail-row">
            <label>Source</label>
            <input value="${escapeHtml(source.label)}" readonly>
        </div>

        <div class="detail-row">
            <label>Relationship type</label>
            <input value="${escapeHtml(edge.data("rel_type"))}" readonly>
        </div>

        <div class="detail-row">
            <label>Target</label>
            <input value="${escapeHtml(target.label)}" readonly>
        </div>

        <p class="muted">
            Direction: source → target
        </p>

        <button class="btn" id="btn-open-source">
            Open Source Entity
        </button>

        <button class="btn" id="btn-open-target">
            Open Target Entity
        </button>

        <button class="btn danger" id="btn-delete-relationship">
            Delete Relationship
        </button>
    `;

    $("btn-open-source").onclick = () =>
        showEntityDetail(sourceId).catch(showError);

    $("btn-open-target").onclick = () =>
        showEntityDetail(targetId).catch(showError);

    $("btn-delete-relationship").onclick = async () => {
        if (!confirm("Delete this relationship?")) return;

        await deleteRelationship(id);
    };
}

async function deleteRelationship(id) {
    await safeInvoke("delete_relationship", { id });

    closeDetail();
    await loadGraph();

    setStatus("Relationship deleted.");
}

async function showNeighborhood(id) {
    const result = await safeInvoke("entity_neighborhood", { id });

    const neighbors = result.neighbors || [];

    const html = neighbors.length
        ? neighbors.map(item => `
            <button class="entity-list-item"
                    data-neighbor="${escapeHtml(item.other_id)}">
                <span class="type-badge">
                    ${escapeHtml(item.other_type)}
                </span>
                ${escapeHtml(item.other_label)}
                <small>
                    ${escapeHtml(item.direction)} ·
                    ${escapeHtml(item.rel_type)}
                </small>
            </button>
        `).join("")
        : `<p class="muted">No connected entities.</p>`;

    showModal("Entity Neighbourhood", html, async () => {});

    $("modal-confirm")?.classList.add("hidden");

    document.querySelectorAll("[data-neighbor]").forEach(button => {
        button.addEventListener("click", async () => {
            closeModal();
            await showEntityDetail(button.dataset.neighbor);
        });
    });
}

/* =========================================================
   TEXT EXTRACTION AND SCRAPING
========================================================= */

function showExtractModal() {
    requireCase();

    showModal(
        "Extract Entities",
        `
        <div class="detail-row">
            <label>Paste text to analyze</label>
            <textarea id="extract-text" rows="9"
                placeholder="Paste public-source text here..."></textarea>
        </div>
        `,
        async () => {
            const text = $("extract-text").value.trim();

            if (!text) throw new Error("Text is required.");

            const count = await safeInvoke(
                "extract_entities_from_text",
                { text }
            );

            closeModal();
            await loadGraph();

            alert(`${count} new entities extracted.`);
        }
    );
}

function showScrapeModal() {
    requireCase();

    showModal(
        "Fetch Public Webpage",
        `
        <div class="detail-row">
            <label>URL</label>
            <input id="scrape-url"
                   type="url"
                   placeholder="https://example.com">
        </div>
        <p class="muted">
            Fetches a public webpage and imports extracted entities
            into the active investigation.
        </p>
        `,
        async () => {
            const url = $("scrape-url").value.trim();

            if (!/^https?:\/\//i.test(url)) {
                throw new Error("Enter a valid HTTP or HTTPS URL.");
            }

            const result = await safeInvoke("scrape_and_import", { url });

            closeModal();
            await loadGraph();

            alert(
                `Fetch completed\n\n` +
                `URL: ${result.final_url}\n` +
                `HTTP: ${result.status}\n` +
                `Entities: ${result.entities_added}\n` +
                `Relationships: ${result.relationships_added}`
            );
        }
    );
}

/* =========================================================
   GRAPH SEARCH AND EXPORT
========================================================= */

async function handleSearch(query) {
    if (!cy) return;

    const q = query.trim();

    if (!q || !currentCase) {
        cy.elements().removeClass("faded highlighted");
        return;
    }

    const results = await safeInvoke("search_entities", {
        query: q,
        limit: 100
    });

    const ids = new Set(results.map(item => item.id));

    cy.elements().addClass("faded");
    cy.nodes().filter(node => ids.has(node.id()))
        .removeClass("faded")
        .addClass("highlighted");

    cy.edges().forEach(edge => {
        if (
            ids.has(edge.data("source")) &&
            ids.has(edge.data("target"))
        ) {
            edge.removeClass("faded");
        }
    });
}

function exportPng() {
    if (!cy || !cy.nodes().length) {
        alert("The graph is empty.");
        return;
    }

    const anchor = document.createElement("a");

    anchor.href = cy.png({
        full: true,
        scale: 2,
        bg: "#101316"
    });

    anchor.download =
        `EKUKE-${Date.now()}.png`;

    anchor.click();
}

/* =========================================================
   PLUGINS
========================================================= */

async function loadPlugins() {
    const plugins = await safeInvoke("get_plugins");
    const container = $("plugin-list");

    if (!container) return;

    container.innerHTML = "";

    if (!plugins.length) {
        container.innerHTML =
            `<p class="muted">No plugins installed.</p>`;
        return;
    }

    plugins.forEach(plugin => {
        const button = document.createElement("button");

        button.className = "plugin-item";
        button.type = "button";

        button.innerHTML = `
            <div class="name">${escapeHtml(plugin.name)}</div>
            <div class="desc">${escapeHtml(plugin.description || "")}</div>
        `;

        button.addEventListener("click", async () => {
            if (!activeEntity) {
                alert("Select an entity first.");
                return;
            }

            await showTransformModal(activeEntity);
        });

        container.appendChild(button);
    });
}

async function showTransformModal(entity) {
    const plugins = await safeInvoke("get_plugins");

    const compatible = plugins.filter(plugin =>
        (plugin.input_types || []).includes("*") ||
        (plugin.input_types || []).includes(entity.entity_type)
    );

    if (!compatible.length) {
        alert("No compatible plugins found.");
        return;
    }

    const html = `
        <div class="detail-row">
            <label>Plugin</label>
            <select id="transform-plugin">
                ${compatible.map(plugin => `
                    <option value="${escapeHtml(plugin.id)}">
                        ${escapeHtml(plugin.name)}
                    </option>
                `).join("")}
            </select>
        </div>

        <div id="plugin-config"></div>
    `;

    showModal("Run Intelligence Plugin", html, async () => {
        const pluginId = $("transform-plugin").value;
        const config = {};

        document.querySelectorAll("#plugin-config input").forEach(input => {
            if (input.dataset.name) {
                config[input.dataset.name] = input.value;
            }
        });

        const result = await safeInvoke("run_transform", {
            pluginId,
            entityId: entity.id,
            config
        });

        closeModal();
        await loadGraph();

        alert(
            `${result.entities} entities and ` +
            `${result.relationships} relationships processed.`
        );
    });

    const updateConfig = () => {
        const selected = compatible.find(p =>
            p.id === $("transform-plugin").value
        );

        $("plugin-config").innerHTML =
            (selected?.config_fields || []).map(field => `
                <div class="detail-row">
                    <label>${escapeHtml(field.name)}</label>
                    <input data-name="${escapeHtml(field.name)}"
                           value="${escapeHtml(field.default || "")}">
                </div>
            `).join("");
    };

    $("transform-plugin").addEventListener("change", updateConfig);
    updateConfig();
}

/* =========================================================
   SETTINGS
========================================================= */

async function loadConfig() {
    settingsConfig = await safeInvoke("get_config");
    return settingsConfig;
}

async function showSettings() {
    const config = await loadConfig();

    $("set-ai-enabled").value = String(!!config.ai_enabled);
    $("set-ai-url").value = config.ai_base_url || "";
    $("set-ai-key").value = config.ai_api_key || "";
    $("set-ai-model").value = config.ai_model || "";
    $("set-author").value = config.default_author || "";
    $("set-plugins-dir").value = config.plugins_dir || "";
    $("set-cases-dir").value = config.cases_dir || "";

    $("set-key-shodan").value =
        config.api_keys?.shodan || "";

    $("settings-overlay").classList.remove("hidden");
}

function closeSettings() {
    $("settings-overlay")?.classList.add("hidden");
}

async function saveSettings() {
    const config = settingsConfig || {};

    config.ai_enabled = $("set-ai-enabled").value === "true";
    config.ai_base_url = $("set-ai-url").value.trim();
    config.ai_api_key = $("set-ai-key").value.trim();
    config.ai_model = $("set-ai-model").value.trim();
    config.default_author = $("set-author").value.trim();
    config.plugins_dir = $("set-plugins-dir").value.trim();
    config.cases_dir = $("set-cases-dir").value.trim();

    config.api_keys = config.api_keys || {};

    const shodan = $("set-key-shodan").value.trim();

    if (shodan) config.api_keys.shodan = shodan;

    await safeInvoke("set_config", { config });

    settingsConfig = config;

    closeSettings();
    await loadPlugins();

    setStatus("Settings saved.");
}

async function saveExtraKey() {
    const name = $("set-key-name").value.trim();
    const value = $("set-key-value").value.trim();

    if (!name || !value) {
        throw new Error("Enter both the key name and value.");
    }

    settingsConfig.api_keys = settingsConfig.api_keys || {};
    settingsConfig.api_keys[name] = value;

    await safeInvoke("set_config", {
        config: settingsConfig
    });

    $("set-key-name").value = "";
    $("set-key-value").value = "";

    alert("API key saved.");
}

async function testAiConnection() {
    const config = settingsConfig || await loadConfig();

    config.ai_enabled = true;
    config.ai_base_url = $("set-ai-url").value.trim();
    config.ai_api_key = $("set-ai-key").value.trim();
    config.ai_model = $("set-ai-model").value.trim();

    await safeInvoke("set_config", { config });

    const output = $("ai-test-result");
    output.textContent = "Testing...";

    try {
        const result = await safeInvoke("ai_test_connection");
        output.textContent = "Connection successful: " + result;
    } catch (error) {
        output.textContent = "Connection failed: " + error;
    }
}

/* =========================================================
   AI ASSISTANT
========================================================= */

function toggleAiPanel() {
    $("ai-panel")?.classList.toggle("hidden");
}

function appendAiMessage(role, message) {
    const container = $("ai-messages");
    if (!container) return;

    const item = document.createElement("div");

    item.className = `ai-msg ai-${role}`;
    item.textContent = message;

    container.appendChild(item);
    container.scrollTop = container.scrollHeight;
}

function removeLastAiMessage() {
    $("ai-messages")?.lastElementChild?.remove();
}

async function sendAiMessage() {
    const input = $("ai-input");
    const message = input.value.trim();

    if (!message) return;

    if (!currentCase) {
        appendAiMessage("system", "Open an investigation first.");
        return;
    }

    input.value = "";

    appendAiMessage("user", message);

    aiHistory.push({
        role: "user",
        content: message
    });

    appendAiMessage("system", "Processing...");

    try {
        const result = await safeInvoke("ai_chat", {
            messages: aiHistory
        });

        removeLastAiMessage();

        const reply = result.reply || "No response.";

        appendAiMessage("assistant", reply);

        aiHistory.push({
            role: "assistant",
            content: reply
        });

        if (result.actions?.length) {
            await loadGraph();
        }
    } catch (error) {
        removeLastAiMessage();
        appendAiMessage("system", "Error: " + error);
    }
}

async function summarizeCase() {
    if (!currentCase) {
        appendAiMessage("system", "Open a case first.");
        return;
    }

    appendAiMessage("system", "Analyzing investigation...");

    try {
        const result = await safeInvoke("ai_summarize_case");
        removeLastAiMessage();
        appendAiMessage("assistant", result);
    } catch (error) {
        removeLastAiMessage();
        appendAiMessage("system", "Error: " + error);
    }
}

/* =========================================================
   MODAL HELPERS
========================================================= */

function showModal(title, bodyHtml, onConfirm) {
    $("modal-title").textContent = title;
    $("modal-body").innerHTML = bodyHtml;

    $("modal-overlay").classList.remove("hidden");

    const oldButton = $("modal-confirm");
    const newButton = oldButton.cloneNode(true);

    oldButton.replaceWith(newButton);

    newButton.classList.remove("hidden");

    newButton.addEventListener("click", async () => {
        setBusy(newButton, true, "Working...");

        try {
            await onConfirm();
        } catch (error) {
            showError(error);
        } finally {
            if (newButton.isConnected) {
                setBusy(newButton, false);
            }
        }
    });
}

function closeModal() {
    $("modal-overlay")?.classList.add("hidden");
}

function closeDetail() {
    activeEntity = null;
    $("detail-panel")?.classList.add("hidden");
}

/* =========================================================
   START APPLICATION
========================================================= */

init().catch(error => {
    console.error("EKUKE initialization failed:", error);
    setStatus("Initialization failed: " + error, true);
});