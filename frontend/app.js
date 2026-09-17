const { invoke } = window.__TAURI__.core;
const { open, save } = window.__TAURI__.dialog;

let cy = null;
let currentCase = null;
let aiHistory = [];

const COLORS = {
    Domain: '#3b82f6', Email: '#10b981', IPv4: '#f59e0b', Phone: '#8b5cf6',
    Handle: '#ec4899', Person: '#ef4444', Organization: '#06b6d4',
    URL: '#84cc16', Document: '#f97316', Other: '#6b7280', default: '#888888'
};
function getColor(type) { return COLORS[type] || COLORS.default; }

async function init() {
    const on = (id, fn) => document.getElementById(id).addEventListener('click', fn);
    on('btn-new-case', showNewCaseModal);
    on('btn-open-case', showOpenCaseModal);
    on('btn-export-case', exportCase);
    on('btn-import-case', importCase);
    on('btn-delete-case', deleteCurrentCase);
    on('btn-add-entity', showAddEntityModal);
    on('btn-extract', showExtractModal);
    on('btn-settings', showSettings);
    on('btn-ai-chat', toggleAiPanel);
    on('btn-ai-close', toggleAiPanel);
    on('btn-ai-send', sendAiMessage);
    on('btn-close-detail', closeDetail);
    on('modal-cancel', closeModal);
    on('settings-cancel', closeSettings);
    on('settings-save', saveSettings);
    on('btn-ai-test', testAiConnection);
    on('btn-add-key', saveExtraKey);
    document.querySelectorAll('.tab').forEach(t => t.addEventListener('click', () => {
        document.querySelectorAll('.tab').forEach(x => x.classList.remove('active'));
        document.querySelectorAll('.tab-body').forEach(x => x.classList.add('hidden'));
        t.classList.add('active');
        document.getElementById(t.dataset.tab).classList.remove('hidden');
    }));
    document.getElementById('ai-input').addEventListener('keydown', e => {
        if (e.key === 'Enter') sendAiMessage();
    });
    document.getElementById('search-bar').addEventListener('input', debounce(handleSearch, 300));

    initCytoscape();
    await loadPlugins();
    await loadConfig();
    await checkOpenCase();
}

function initCytoscape() {
    cy = cytoscape({
        container: document.getElementById('graph-container'),
        style: [
            { selector: 'node', style: {
                'background-color': 'data(color)', 'label': 'data(label)',
                'width': 40, 'height': 40, 'font-size': '10px', 'color': '#e0e0e0',
                'text-valign': 'bottom', 'text-margin-y': 6,
                'border-width': 2, 'border-color': '#333'
            }},
            { selector: 'edge', style: {
                'width': 2, 'line-color': '#555', 'target-arrow-color': '#555',
                'target-arrow-shape': 'triangle', 'curve-style': 'bezier',
                'label': 'data(label)', 'font-size': '9px', 'color': '#888',
                'text-rotation': 'autorotate'
            }},
            { selector: 'node:selected', style: { 'border-color': '#ff3333', 'border-width': 3 } }
        ],
        layout: { name: 'cose', animate: false }
    });
    cy.on('tap', 'node', evt => showEntityDetail(evt.target.id()));
    cy.on('cxttap', 'node', evt => {
        const id = evt.target.id();
        if (!confirm('Delete this entity?')) return;
        invoke('delete_entity', { id }).then(loadGraph);
    });
}

async function loadGraph() {
    if (!currentCase) return;
    const entities = await invoke('get_entities', {});
    const rels = await invoke('get_relationships', {});
    const nodeIds = new Set(entities.map(e => e.id));
    cy.elements().remove();
    entities.forEach(e => {
        cy.add({ group: 'nodes', data: {
            id: e.id, label: e.label, color: getColor(e.entity_type),
            type: e.entity_type
        }});
    });
    rels.forEach(r => {
        if (nodeIds.has(r.source_id) && nodeIds.has(r.target_id)) {
            cy.add({ group: 'edges', data: {
                id: r.id, source: r.source_id, target: r.target_id, label: r.rel_type
            }});
        }
    });
    cy.layout({ name: 'cose', animate: false }).run();
}

async function checkOpenCase() {
    currentCase = await invoke('get_current_case');
    if (currentCase) {
        document.getElementById('case-name').textContent = `Case: ${currentCase.name}`;
        await loadGraph();
    }
}

// ─── Cases ────────────────────────────────────────────
function showNewCaseModal() {
    showModal('New Case', `
        <div class="detail-row"><label>Case Name</label>
        <input type="text" id="case-name-input" placeholder="Investigation name"></div>
    `, async () => {
        const name = document.getElementById('case-name-input').value.trim();
        if (!name) throw new Error('Name is required');
        currentCase = await invoke('create_case', { name });
        document.getElementById('case-name').textContent = `Case: ${currentCase.name}`;
        await loadGraph();
    });
}

async function showOpenCaseModal() {
    const cases = await invoke('list_cases');
    let html = '<div class="entity-list">';
    if (cases.length === 0) html += '<p class="muted">No cases found.</p>';
    cases.forEach(c => {
        html += `<div class="entity-list-item" data-id="${c.id}" data-name="${escapeHtml(c.name)}">
            <span class="type-badge">${c.status}</span> ${escapeHtml(c.name)}</div>`;
    });
    html += '</div>';
    showModal('Open Case', html, async () => {});
    document.querySelectorAll('#modal .entity-list-item').forEach(item => {
        item.addEventListener('click', async () => {
            currentCase = await invoke('open_case', { caseId: item.dataset.id });
            document.getElementById('case-name').textContent = `Case: ${currentCase.name}`;
            closeModal();
            await loadGraph();
        });
    });
}

async function deleteCurrentCase() {
    if (!currentCase) return alert('No case open');
    if (!confirm(`Delete case "${currentCase.name}" and ALL its data? This cannot be undone.`)) return;
    await invoke('delete_case', { caseId: currentCase.id });
    currentCase = null;
    document.getElementById('case-name').textContent = '';
    cy.elements().remove();
}

async function exportCase() {
    if (!currentCase) return alert('No case open');
    const path = await invoke('export_case', { caseId: currentCase.id });
    alert(`Exported to:\n${path}`);
}

async function importCase() {
    const path = await open({ filters: [{ name: 'Ekuke Case', extensions: ['ekuke'] }] });
    if (!path) return;
    currentCase = await invoke('import_case', { path });
    document.getElementById('case-name').textContent = `Case: ${currentCase.name}`;
    await loadGraph();
    alert('Case imported');
}

// ─── Entities ─────────────────────────────────────────
function showAddEntityModal() {
    if (!currentCase) return alert('Open or create a case first');
    showModal('Add Entity', `
        <div class="detail-row"><label>Type</label>
            <select id="new-entity-type">
                ${['Domain','Email','IPv4','Phone','Handle','Person','Organization','URL','Document','Other']
                    .map(t => `<option>${t}</option>`).join('')}
            </select></div>
        <div class="detail-row"><label>Label</label>
            <input type="text" id="new-entity-label" placeholder="example.com"></div>
    `, async () => {
        const entity_type = document.getElementById('new-entity-type').value;
        const label = document.getElementById('new-entity-label').value.trim();
        if (!label) throw new Error('Label is required');
        await invoke('create_entity', { entityType: entity_type, label, properties: null });
        await loadGraph();
    });
}

function showExtractModal() {
    if (!currentCase) return alert('Open or create a case first');
    showModal('Extract Entities from Text', `
        <div class="detail-row"><label>Paste text (emails, IPs, domains, phones, handles are auto-detected)</label>
        <textarea id="extract-text" rows="8"></textarea></div>
    `, async () => {
        const text = document.getElementById('extract-text').value;
        const added = await invoke('extract_entities_from_text', { text });
        await loadGraph();
        alert(`Extracted ${added} new entities`);
    });
}

async function showEntityDetail(id) {
    const entity = await invoke('get_entity_by_id', { id });
    const panel = document.getElementById('detail-panel');
    panel.classList.remove('hidden');
    let propsHtml = '';
    for (const [k, v] of Object.entries(entity.properties || {})) {
        propsHtml += `<div class="detail-row"><label>${escapeHtml(k)}</label>
            <input type="text" value="${escapeHtml(typeof v === 'string' ? v : JSON.stringify(v))}"
                data-prop="${escapeHtml(k)}"></div>`;
    }
    document.getElementById('detail-content').innerHTML = `
        <div class="detail-row"><label>ID</label><input type="text" value="${entity.id}" readonly></div>
        <div class="detail-row"><label>Type</label><input type="text" value="${escapeHtml(entity.entity_type)}" readonly></div>
        <div class="detail-row"><label>Label</label><input type="text" id="detail-label" value="${escapeHtml(entity.label)}"></div>
        ${propsHtml}
        <div class="detail-row"><label>Link to (label)</label>
            <input type="text" id="link-target" placeholder="other entity label">
            <input type="text" id="link-type" placeholder="rel type e.g. resolves_to"></div>
        <div style="margin-top:16px;">
            <button class="btn primary" id="btn-update-entity">Update</button>
            <button class="btn" id="btn-create-link">Create Link</button>
            <button class="btn danger" id="btn-delete-entity">Delete</button>
            <button class="btn" id="btn-run-transform">Run Plugin</button>
        </div>`;
    document.getElementById('btn-update-entity').addEventListener('click', async () => {
        const label = document.getElementById('detail-label').value;
        const properties = {};
        document.querySelectorAll('#detail-content input[data-prop]').forEach(inp => {
            properties[inp.dataset.prop] = inp.value;
        });
        await invoke('update_entity', { id: entity.id, label, properties });
        await loadGraph();
    });
    document.getElementById('btn-create-link').addEventListener('click', async () => {
        const targetLabel = document.getElementById('link-target').value.trim();
        const relType = document.getElementById('link-type').value.trim() || 'linked_to';
        const matches = await invoke('search_entities', { query: targetLabel, limit: 1 });
        if (matches.length === 0) return alert('Target entity not found');
        await invoke('create_relationship', {
            sourceId: entity.id, targetId: matches[0].id, relType, properties: null
        });
        await loadGraph();
    });
    document.getElementById('btn-delete-entity').addEventListener('click', async () => {
        if (!confirm('Delete this entity?')) return;
        await invoke('delete_entity', { id: entity.id });
        closeDetail();
        await loadGraph();
    });
    document.getElementById('btn-run-transform').addEventListener('click', () => showTransformModal(entity));
}

async function showTransformModal(entity) {
    const plugins = await invoke('get_plugins');
    const compatible = plugins.filter(p =>
        (p.input_types || []).includes(entity.entity_type) || (p.input_types || []).includes('*'));
    if (compatible.length === 0) return alert('No compatible plugins installed');
    let html = '<div class="detail-row"><label>Select Plugin</label><select id="transform-plugin">';
    compatible.forEach(p => {
        html += `<option value="${p.id}">${escapeHtml(p.name)} — ${escapeHtml(p.description)}</option>`;
    });
    html += '</select></div><div id="plugin-config"></div>';
    showModal('Run Plugin', html, async () => {
        const pluginId = document.getElementById('transform-plugin').value;
        const config = {};
        document.querySelectorAll('#plugin-config input').forEach(inp => {
            if (inp.dataset.name) config[inp.dataset.name] = inp.value;
        });
        try {
            const result = await invoke('run_transform', { pluginId, entityId: entity.id, config });
            await loadGraph();
            closeModal();
            alert(`Done: ${result.entities} entities, ${result.relationships} relationships added`);
        } catch (e) {
            alert('Plugin failed: ' + e);
        }
    });
    const updateConfig = () => {
        const pid = document.getElementById('transform-plugin').value;
        const plugin = compatible.find(p => p.id === pid);
        const container = document.getElementById('plugin-config');
        container.innerHTML = '';
        (plugin?.config_fields || []).forEach(f => {
            container.innerHTML += `<div class="detail-row">
                <label>${escapeHtml(f.name)}${f.required ? ' *' : ''}</label>
                <input type="text" data-name="${escapeHtml(f.name)}" value="${escapeHtml(f.default || '')}"
                    placeholder="${escapeHtml(f.description || '')}"></div>`;
        });
    };
    document.getElementById('transform-plugin').addEventListener('change', updateConfig);
    updateConfig();
}

// ─── Search ───────────────────────────────────────────
async function handleSearch(e) {
    const query = e.target.value.trim();
    if (!query || !currentCase || !cy) return;
    if (!query) { cy.nodes().style('opacity', 1); return; }
    const results = await invoke('search_entities', { query, limit: 20 });
    const ids = results.map(r => r.id);
    cy.nodes().forEach(n => n.style('opacity', ids.includes(n.id()) ? 1 : 0.15));
}

// ─── Plugins list ─────────────────────────────────────
async function loadPlugins() {
    try {
        const plugins = await invoke('get_plugins');
        const container = document.getElementById('plugin-list');
        container.innerHTML = plugins.length ? '' : '<p class="muted">No plugins found in plugins dir.</p>';
        plugins.forEach(p => {
            const div = document.createElement('div');
            div.className = 'plugin-item';
            div.innerHTML = `<div class="name">${escapeHtml(p.name)}</div>
                <div class="desc">${escapeHtml(p.description || '')}</div>`;
            container.appendChild(div);
        });
    } catch (e) { console.error('Failed to load plugins:', e); }
}

// ─── Settings ─────────────────────────────────────────
let settingsConfig = null;
async function loadConfig() {
    settingsConfig = await invoke('get_config');
}
async function showSettings() {
    await loadConfig();
    const c = settingsConfig;
    document.getElementById('set-ai-enabled').value = c.ai_enabled ? 'true' : 'false';
    document.getElementById('set-ai-url').value = c.ai_base_url || '';
    document.getElementById('set-ai-key').value = c.ai_api_key || '';
    document.getElementById('set-ai-model').value = c.ai_model || '';
    document.getElementById('set-author').value = c.default_author || '';
    document.getElementById('set-plugins-dir').value = c.plugins_dir || '';
    document.getElementById('set-cases-dir').value = c.cases_dir || '';
    document.getElementById('set-key-shodan').value = (c.api_keys && c.api_keys.shodan) || '';
    document.getElementById('settings-overlay').classList.remove('hidden');
}
function closeSettings() {
    document.getElementById('settings-overlay').classList.add('hidden');
}
async function saveSettings() {
    const c = settingsConfig;
    c.ai_enabled = document.getElementById('set-ai-enabled').value === 'true';
    c.ai_base_url = document.getElementById('set-ai-url').value.trim() || 'https://api.openai.com/v1';
    c.ai_api_key = document.getElementById('set-ai-key').value.trim();
    c.ai_model = document.getElementById('set-ai-model').value.trim() || 'gpt-4o-mini';
    c.default_author = document.getElementById('set-author').value.trim() || 'Anonymous-beta';
    c.plugins_dir = document.getElementById('set-plugins-dir').value.trim() || c.plugins_dir;
    c.cases_dir = document.getElementById('set-cases-dir').value.trim() || c.cases_dir;
    const shodan = document.getElementById('set-key-shodan').value.trim();
    c.api_keys = c.api_keys || {};
    if (shodan) c.api_keys.shodan = shodan;
    await invoke('set_config', { config: c });
    closeSettings();
    await loadPlugins();
    appendAiMessage('system', 'Settings saved.');
}
async function saveExtraKey() {
    const name = document.getElementById('set-key-name').value.trim();
    const value = document.getElementById('set-key-value').value.trim();
    if (!name || !value) return alert('Key name and value required');
    settingsConfig.api_keys = settingsConfig.api_keys || {};
    settingsConfig.api_keys[name] = value;
    await invoke('set_config', { config: settingsConfig });
    document.getElementById('set-key-name').value = '';
    document.getElementById('set-key-value').value = '';
    alert(`Key "${name}" saved`);
}
async function testAiConnection() {
    // save current AI fields first so the test uses them
    settingsConfig.ai_enabled = true;
    settingsConfig.ai_base_url = document.getElementById('set-ai-url').value.trim() || 'https://api.openai.com/v1';
    settingsConfig.ai_api_key = document.getElementById('set-ai-key').value.trim();
    settingsConfig.ai_model = document.getElementById('set-ai-model').value.trim() || 'gpt-4o-mini';
    await invoke('set_config', { config: settingsConfig });
    const el = document.getElementById('ai-test-result');
    el.textContent = 'Testing...';
    try {
        const msg = await invoke('ai_test_connection');
        el.textContent = '✔ ' + msg;
    } catch (e) {
        el.textContent = '✘ ' + e;
    }
}

// ─── AI chat ──────────────────────────────────────────
function toggleAiPanel() {
    document.getElementById('ai-panel').classList.toggle('hidden');
}
function appendAiMessage(role, text) {
    const box = document.getElementById('ai-messages');
    const div = document.createElement('div');
    div.className = `ai-msg ai-${role}`;
    div.textContent = text;
    box.appendChild(div);
    box.scrollTop = box.scrollHeight;
}
async function sendAiMessage() {
    const input = document.getElementById('ai-input');
    const text = input.value.trim();
    if (!text) return;
    if (!currentCase) { appendAiMessage('system', '⚠ Open or create a case first so the AI has data to work on.'); return; }
    input.value = '';
    appendAiMessage('user', text);
    aiHistory.push({ role: 'user', content: text });
    appendAiMessage('system', '…thinking');
    try {
        const result = await invoke('ai_chat', { messages: aiHistory });
        document.getElementById('ai-messages').lastChild.remove(); // remove thinking
        appendAiMessage('assistant', result.reply || '(empty reply)');
        aiHistory.push({ role: 'assistant', content: result.reply || '' });
        if (result.actions && result.actions.length > 0) {
            appendAiMessage('system', `🔧 Performed ${result.actions.length} action(s): ` +
                result.actions.map(a => a.tool).join(', '));
            await loadGraph();
        }
    } catch (e) {
        document.getElementById('ai-messages').lastChild.remove();
        appendAiMessage('system', 'Error: ' + e);
    }
}

// ─── Modal helpers ────────────────────────────────────
function showModal(title, bodyHtml, onConfirm) {
    document.getElementById('modal-title').textContent = title;
    document.getElementById('modal-body').innerHTML = bodyHtml;
    document.getElementById('modal-overlay').classList.remove('hidden');
    const confirmBtn = document.getElementById('modal-confirm');
    const newConfirm = confirmBtn.cloneNode(true);
    confirmBtn.parentNode.replaceChild(newConfirm, confirmBtn);
    newConfirm.addEventListener('click', async () => {
        try { await onConfirm(); } catch (e) { alert('Error: ' + e); }
    });
}
function closeModal() { document.getElementById('modal-overlay').classList.add('hidden'); }
function closeDetail() { document.getElementById('detail-panel').classList.add('hidden'); }

function debounce(fn, ms) {
    let timeout;
    return (...args) => { clearTimeout(timeout); timeout = setTimeout(() => fn(...args), ms); };
}
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text == null ? '' : String(text);
    return div.innerHTML;
}

init();