const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

let cy = null;
let currentCase = null;
let selectedEntityId = null;

const COLORS = {
    Domain: '#3b82f6',
    Email: '#10b981',
    IPv4: '#f59e0b',
    Phone: '#8b5cf6',
    Handle: '#ec4899',
    Person: '#ef4444',
    Organization: '#06b6d4',
    URL: '#84cc16',
    Document: '#f97316',
    Other: '#6b7280',
    default: '#888888'
};

function getColor(type) {
    return COLORS[type] || COLORS.default;
}

async function init() {
    document.getElementById('btn-new-case').addEventListener('click', showNewCaseModal);
    document.getElementById('btn-open-case').addEventListener('click', showOpenCaseModal);
    document.getElementById('btn-save-case').addEventListener('click', saveCase);
    document.getElementById('btn-export-case').addEventListener('click', exportCase);
    document.getElementById('btn-import-case').addEventListener('click', importCase);
    document.getElementById('btn-add-entity').addEventListener('click', showAddEntityModal);
    document.getElementById('btn-extract').addEventListener('click', showExtractModal);
    document.getElementById('search-bar').addEventListener('input', debounce(handleSearch, 300));
    document.getElementById('btn-close-detail').addEventListener('click', closeDetail);
    document.getElementById('modal-cancel').addEventListener('click', closeModal);
    
    initCytoscape();
    await loadPlugins();
    await checkOpenCase();
}

function initCytoscape() {
    cy = cytoscape({
        container: document.getElementById('graph-container'),
        style: [
            {
                selector: 'node',
                style: {
                    'background-color': 'data(color)',
                    'label': 'data(label)',
                    'width': 40,
                    'height': 40,
                    'font-size': '10px',
                    'color': '#e0e0e0',
                    'text-outline-color': '#0d0d0d',
                    'text-outline-width': 2,
                    'text-valign': 'bottom',
                    'text-halign': 'center',
                    'text-margin-y': 4
                }
            },
            {
                selector: 'edge',
                style: {
                    'width': 2,
                    'line-color': '#444',
                    'target-arrow-color': '#444',
                    'target-arrow-shape': 'triangle',
                    'curve-style': 'bezier',
                    'label': 'data(label)',
                    'font-size': '9px',
                    'color': '#888',
                    'text-outline-color': '#0d0d0d',
                    'text-outline-width': 1
                }
            },
            {
                selector: ':selected',
                style: {
                    'border-width': 3,
                    'border-color': '#ff3333'
                }
            }
        ],
        layout: { name: 'grid' },
        minZoom: 0.2,
        maxZoom: 3
    });

    cy.on('tap', 'node', async (evt) => {
        selectedEntityId = evt.target.id();
        await showEntityDetail(selectedEntityId);
    });

    cy.on('tap', (evt) => {
        if (evt.target === cy) {
            closeDetail();
        }
    });
}

async function checkOpenCase() {
    const info = await invoke('get_case_info');
    if (info) {
        currentCase = info;
        document.getElementById('case-name').textContent = info.name;
        await loadGraph();
    }
}

async function loadGraph() {
    if (!cy) return;
    cy.elements().remove();
    
    const entities = await invoke('get_entities');
    const relationships = await invoke('get_relationships');
    
    const nodes = entities.map(e => ({
        data: {
            id: e.id,
            label: e.label,
            color: getColor(e.entity_type),
            entity: e
        }
    }));
    
    const edges = relationships.map(r => ({
        data: {
            id: r.id,
            source: r.source_id,
            target: r.target_id,
            label: r.rel_type,
            relationship: r
        }
    }));
    
    cy.add([...nodes, ...edges]);
    
    const layout = cy.layout({
        name: 'cose',
        padding: 30,
        nodeRepulsion: 400000,
        idealEdgeLength: 100,
        animate: true
    });
    layout.run();
    
    updateEntityList(entities);
}

function updateEntityList(entities) {
    const container = document.getElementById('entity-types');
    const types = {};
    entities.forEach(e => {
        if (!types[e.entity_type]) types[e.entity_type] = [];
        types[e.entity_type].push(e);
    });
    
    container.innerHTML = '';
    for (const [type, list] of Object.entries(types)) {
        const section = document.createElement('div');
        section.innerHTML = `<div style="font-size:11px;color:#888;margin:8px 0 4px;text-transform:uppercase;">${type} (${list.length})</div>`;
        list.forEach(e => {
            const item = document.createElement('div');
            item.className = 'entity-list-item';
            item.innerHTML = `<span class="entity-type-badge type-${type.toLowerCase()}">${type[0]}</span>${escapeHtml(e.label)}`;
            item.addEventListener('click', () => {
                const node = cy.getElementById(e.id);
                if (node.length) {
                    cy.fit(node, 100);
                    node.select();
                    showEntityDetail(e.id);
                }
            });
            section.appendChild(item);
        });
        container.appendChild(section);
    }
}

async function showNewCaseModal() {
    showModal('New Case', `
        <div class="detail-row"><label>Case Name</label><input type="text" id="case-name-input"></div>
        <div class="detail-row"><label>Description</label><textarea id="case-desc-input"></textarea></div>
        <div class="detail-row"><label>Author</label><input type="text" id="case-author-input" value="Anonymous-beta"></div>
        <div class="detail-row"><label>Password (encrypts case)</label><input type="password" id="case-pass-input"></div>
    `, async () => {
        const name = document.getElementById('case-name-input').value;
        const desc = document.getElementById('case-desc-input').value;
        const author = document.getElementById('case-author-input').value;
        const pass = document.getElementById('case-pass-input').value;
        if (!name || !pass) return alert('Name and password required');
        
        const meta = await invoke('create_case', { name, description: desc, author, password: pass });
        currentCase = meta;
        document.getElementById('case-name').textContent = meta.name;
        closeModal();
        await loadGraph();
    });
}

async function showOpenCaseModal() {
    const path = await open({ directory: true });
    if (!path) return;
    const pass = prompt('Enter case password:');
    if (!pass) return;
    
    try {
        const meta = await invoke('open_case', { casePath: path, password: pass });
        currentCase = meta;
        document.getElementById('case-name').textContent = meta.name;
        await loadGraph();
    } catch (e) {
        alert('Failed to open case: ' + e);
    }
}

async function saveCase() {
    if (!currentCase) return alert('No case open');
    const pass = prompt('Enter password to save:');
    if (!pass) return;
    try {
        await invoke('save_case', { password: pass });
        alert('Case saved');
    } catch (e) {
        alert('Save failed: ' + e);
    }
}

async function exportCase() {
    if (!currentCase) return alert('No case open');
    const pass = prompt('Enter password to encrypt export:');
    if (!pass) return;
    try {
        const path = await invoke('export_case', { password: pass });
        alert('Exported to: ' + path);
    } catch (e) {
        alert('Export failed: ' + e);
    }
}

async function importCase() {
    const path = await open({ filters: [{ name: 'EKUKE Case', extensions: ['ekuke'] }] });
    if (!path) return;
    const pass = prompt('Enter password to decrypt:');
    if (!pass) return;
    try {
        const meta = await invoke('import_case', { ekukePath: path, password: pass });
        currentCase = meta;
        document.getElementById('case-name').textContent = meta.name;
        await loadGraph();
    } catch (e) {
        alert('Import failed: ' + e);
    }
}

async function showAddEntityModal() {
    if (!currentCase) return alert('Open a case first');
    showModal('Add Entity', `
        <div class="detail-row"><label>Type</label>
            <select id="entity-type-input">
                <option>Person</option>
                <option>Organization</option>
                <option>Domain</option>
                <option>Email</option>
                <option>IPv4</option>
                <option>Phone</option>
                <option>Handle</option>
                <option>URL</option>
                <option>Document</option>
                <option>Other</option>
            </select>
        </div>
        <div class="detail-row"><label>Label</label><input type="text" id="entity-label-input"></div>
        <div class="detail-row"><label>Properties (JSON)</label><textarea id="entity-props-input">{}</textarea></div>
    `, async () => {
        const type = document.getElementById('entity-type-input').value;
        const label = document.getElementById('entity-label-input').value;
        const props = JSON.parse(document.getElementById('entity-props-input').value || '{}');
        const entity = await invoke('add_entity', { entityType: type, label, properties: props });
        closeModal();
        await loadGraph();
        cy.getElementById(entity.id).select();
        showEntityDetail(entity.id);
    });
}

async function showExtractModal() {
    if (!currentCase) return alert('Open a case first');
    showModal('Extract from Text', `
        <div class="detail-row"><label>Paste text / paste</label><textarea id="extract-text-input" style="min-height:200px;"></textarea></div>
    `, async () => {
        const text = document.getElementById('extract-text-input').value;
        const entities = await invoke('extract_entities_from_text', { text });
        closeModal();
        if (entities.length === 0) return alert('No entities found');
        
        let added = 0;
        for (const e of entities) {
            try {
                await invoke('add_entity', {
                    entityType: e.entity_type,
                    label: e.label,
                    properties: e.properties
                });
                added++;
            } catch (err) {
                // duplicate or error, skip
            }
        }
        await loadGraph();
        alert(`Extracted ${added} entities`);
    });
}

async function showEntityDetail(id) {
    const entity = await invoke('get_entity_by_id', { id });
    if (!entity) return;
    
    const panel = document.getElementById('detail-panel');
    panel.classList.remove('hidden');
    
    let propsHtml = '';
    for (const [k, v] of Object.entries(entity.properties)) {
        propsHtml += `<div class="detail-row"><label>${escapeHtml(k)}</label><input type="text" value="${escapeHtml(String(v))}" data-prop="${escapeHtml(k)}"></div>`;
    }
    
    document.getElementById('detail-content').innerHTML = `
        <div class="detail-row"><label>ID</label><input type="text" value="${entity.id}" readonly></div>
        <div class="detail-row"><label>Type</label><input type="text" value="${entity.entity_type}" readonly></div>
        <div class="detail-row"><label>Label</label><input type="text" id="detail-label" value="${escapeHtml(entity.label)}"></div>
        <div class="detail-row"><label>Created</label><input type="text" value="${entity.created_at}" readonly></div>
        ${propsHtml}
        <div style="margin-top:16px;">
            <button class="btn primary" id="btn-update-entity">Update</button>
            <button class="btn" id="btn-delete-entity" style="background:#331111;border-color:#ff3333;color:#ff3333;">Delete</button>
            <button class="btn" id="btn-run-transform">Run Transform</button>
        </div>
    `;
    
    document.getElementById('btn-update-entity').addEventListener('click', async () => {
        const label = document.getElementById('detail-label').value;
        const props = {};
        document.querySelectorAll('#detail-content input[data-prop]').forEach(input => {
            props[input.dataset.prop] = input.value;
        });
        await invoke('update_entity', { id: entity.id, label, properties: props });
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
    const compatible = plugins.filter(p => p.input_types.includes(entity.entity_type) || p.input_types.includes('*'));
    
    if (compatible.length === 0) return alert('No compatible plugins');
    
    let html = '<div class="detail-row"><label>Select Plugin</label><select id="transform-plugin">';
    compatible.forEach(p => {
        html += `<option value="${p.id}">${p.name} — ${p.description}</option>`;
    });
    html += '</select></div>';
    
    html += '<div id="plugin-config"></div>';
    
    showModal('Run Transform', html, async () => {
        const pluginId = document.getElementById('transform-plugin').value;
        const config = {};
        document.querySelectorAll('#plugin-config input').forEach(input => {
            config[input.dataset.name] = input.value;
        });
        
        try {
            const result = await invoke('run_transform', { pluginId, entityId: entity.id, config });
            await loadGraph();
            closeModal();
            alert(`Transform complete: ${result.entities.length} entities, ${result.relationships.length} relationships`);
        } catch (e) {
            alert('Transform failed: ' + e);
        }
    });
    
    document.getElementById('transform-plugin').addEventListener('change', updatePluginConfig);
    updatePluginConfig();
    
    function updatePluginConfig() {
        const pid = document.getElementById('transform-plugin').value;
        const plugin = compatible.find(p => p.id === pid);
        const container = document.getElementById('plugin-config');
        container.innerHTML = '';
        if (!plugin || !plugin.config_fields) return;
        plugin.config_fields.forEach(f => {
            container.innerHTML += `
                <div class="detail-row">
                    <label>${f.name}${f.required ? ' *' : ''}</label>
                    <input type="text" data-name="${f.name}" value="${f.default || ''}" placeholder="${f.description}">
                </div>
            `;
        });
    }
}

async function handleSearch(e) {
    const query = e.target.value.trim();
    if (!query || !currentCase) return;
    
    const results = await invoke('search_entities', { query, limit: 20 });
    if (results.length > 0 && cy) {
        const ids = results.map(r => r.id);
        cy.nodes().forEach(n => {
            if (ids.includes(n.id())) {
                n.style('opacity', 1);
            } else {
                n.style('opacity', 0.15);
            }
        });
    } else if (cy) {
        cy.nodes().style('opacity', 1);
    }
}

async function loadPlugins() {
    try {
        const plugins = await invoke('get_plugins');
        const container = document.getElementById('plugin-list');
        container.innerHTML = '';
        plugins.forEach(p => {
            const div = document.createElement('div');
            div.className = 'plugin-item';
            div.innerHTML = `<div class="name">${escapeHtml(p.name)}</div><div class="desc">${escapeHtml(p.description)}</div>`;
            container.appendChild(div);
        });
    } catch (e) {
        console.error('Failed to load plugins:', e);
    }
}

function closeDetail() {
    document.getElementById('detail-panel').classList.add('hidden');
    selectedEntityId = null;
    if (cy) cy.$(':selected').unselect();
}

function showModal(title, bodyHtml, onConfirm) {
    document.getElementById('modal-title').textContent = title;
    document.getElementById('modal-body').innerHTML = bodyHtml;
    document.getElementById('modal-overlay').classList.remove('hidden');
    
    const confirmBtn = document.getElementById('modal-confirm');
    const newConfirm = confirmBtn.cloneNode(true);
    confirmBtn.parentNode.replaceChild(newConfirm, confirmBtn);
    newConfirm.addEventListener('click', async () => {
        try {
            await onConfirm();
        } catch (e) {
            alert('Error: ' + e);
        }
    });
}

function closeModal() {
    document.getElementById('modal-overlay').classList.add('hidden');
}

function debounce(fn, ms) {
    let timeout;
    return (...args) => {
        clearTimeout(timeout);
        timeout = setTimeout(() => fn(...args), ms);
    };
}

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

init();
