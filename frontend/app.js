// Geist VPN - Frontend Application Logic

const { invoke } = window.__TAURI__.tauri;

// DOM Elements
const profileSelect = document.getElementById('profile-select');
const connectBtn = document.getElementById('connect-btn');
const connectSpinner = document.getElementById('connect-spinner');
const statusIndicator = document.getElementById('status-indicator');
const statusText = document.getElementById('status-text');
const connectionInfo = document.getElementById('connection-info');
const profilesList = document.getElementById('profiles-list');

const profileModal = document.getElementById('profile-modal');
const profileForm = document.getElementById('profile-form');
const modalTitle = document.getElementById('modal-title');
const profileNameInput = document.getElementById('profile-name');
const profileHostInput = document.getElementById('profile-host');
const profilePortInput = document.getElementById('profile-port');
const profileProtocolSelect = document.getElementById('profile-protocol');
const profileAccountInput = document.getElementById('profile-account');
const profileTimeoutInput = document.getElementById('profile-timeout');
const saveProfileBtn = document.getElementById('save-profile-btn');
const saveSpinner = document.getElementById('save-spinner');

const statusModal = document.getElementById('status-modal');
const statusDetails = document.getElementById('status-details');

// Application State
let currentProfiles = [];
let allProfiles = [];
let currentStatus = { connected: false, profile_name: null, status_message: 'Disconnected' };
let editingProfileId = null;
let currentView = 'all'; // 'all', 'favorites', 'recent'

// Initialize the application
async function init() {
    try {
        console.log('Initializing Geist VPN...');

        // Load profiles
        await loadProfiles();

        // Get initial status
        await updateConnectionStatus();

        // Set up event listeners
        setupEventListeners();

        // Initialize view buttons
        await switchView('all');

        // Start status polling
        startStatusPolling();

        console.log('Geist VPN initialized successfully');
    } catch (error) {
        console.error('Failed to initialize application:', error);
        showError('Failed to initialize application: ' + error);
    }
}

// Event Listeners Setup
function setupEventListeners() {
    // Connection controls
    connectBtn.addEventListener('click', handleConnect);
    profileSelect.addEventListener('change', handleProfileChange);

    // Profile management
    document.getElementById('add-profile-btn').addEventListener('click', () => openProfileModal());
    document.getElementById('modal-close').addEventListener('click', closeProfileModal);
    document.getElementById('cancel-btn').addEventListener('click', closeProfileModal);
    profileForm.addEventListener('submit', handleProfileSubmit);

    // Quick access buttons
    document.getElementById('show-favorites-btn').addEventListener('click', () => switchView('favorites'));
    document.getElementById('show-recent-btn').addEventListener('click', () => switchView('recent'));
    document.getElementById('show-all-btn').addEventListener('click', () => switchView('all'));

    // Modal outside click
    profileModal.addEventListener('click', (e) => {
        if (e.target === profileModal) closeProfileModal();
    });
    statusModal.addEventListener('click', (e) => {
        if (e.target === statusModal) closeStatusModal();
    });

    document.getElementById('close-status-btn').addEventListener('click', closeStatusModal);
}

// Connection Management
async function handleConnect() {
    const profileId = profileSelect.value;
    if (!profileId) {
        showError('Please select a VPN profile first');
        return;
    }

    setConnectButtonLoading(true);

    try {
        const result = invoke('connect_vpn', { profileId });
        updateConnectionStatus(result);
        showSuccess('Connected to VPN successfully (simulated)');
    } catch (error) {
        console.error('Connection failed:', error);
        showError('Failed to connect: ' + error);
        updateConnectionStatus();
    } finally {
        setConnectButtonLoading(false);
    }
}

async function handleDisconnect() {
    setConnectButtonLoading(true);

    try {
        const result = invoke('disconnect_vpn');
        updateConnectionStatus(result);
        showSuccess('Disconnected from VPN (simulated)');
    } catch (error) {
        console.error('Disconnection failed:', error);
        showError('Failed to disconnect: ' + error);
        updateConnectionStatus();
    } finally {
        setConnectButtonLoading(false);
    }
}

// Profile Management
async function loadProfiles() {
    try {
        const result = await invoke('list_profiles');
        allProfiles = result.profiles;

        // Load profiles based on current view
        await loadProfilesForCurrentView();

        updateProfileSelect();
    } catch (error) {
        console.error('Failed to load profiles:', error);
        showError('Failed to load VPN profiles');
    }
}

async function loadProfilesForCurrentView() {
    try {
        let profiles;
        switch (currentView) {
            case 'favorites':
                const favResult = await invoke('get_favorite_profiles');
                profiles = favResult.profiles;
                break;
            case 'recent':
                const recentResult = await invoke('get_recent_profiles', { limit: 10 });
                profiles = recentResult.profiles;
                break;
            default:
                profiles = allProfiles;
        }

        currentProfiles = profiles;
        updateProfilesList();
        updateProfilesTitle();
    } catch (error) {
        console.error('Failed to load profiles for view:', error);
        showError('Failed to load VPN profiles');
    }
}

function updateProfileSelect() {
    profileSelect.innerHTML = '<option value="">Select a profile...</option>';

    currentProfiles.forEach(profile => {
        const option = document.createElement('option');
        option.value = profile.id;
        option.textContent = `${profile.name} (${profile.host})`;
        profileSelect.appendChild(option);
    });
}

function updateProfilesTitle() {
    const titleElement = document.getElementById('profiles-title');
    switch (currentView) {
        case 'favorites':
            titleElement.textContent = '⭐ Favorite VPN Profiles';
            break;
        case 'recent':
            titleElement.textContent = '🕒 Recently Used VPN Profiles';
            break;
        default:
            titleElement.textContent = 'VPN Profiles';
    }
}

function updateProfilesList() {
    if (currentProfiles.length === 0) {
        let emptyMessage = 'No VPN profiles configured yet.';
        switch (currentView) {
            case 'favorites':
                emptyMessage = 'No favorite profiles yet. Mark profiles as favorites to see them here.';
                break;
            case 'recent':
                emptyMessage = 'No recently used profiles yet. Connect to profiles to see them here.';
                break;
        }

        profilesList.innerHTML = `
            <div class="empty-state">
                <p>${emptyMessage}</p>
                ${currentView === 'all' ? '<p>Click "Add Profile" to get started.</p>' : ''}
            </div>
        `;
        return;
    }

    profilesList.innerHTML = '';

    currentProfiles.forEach(profile => {
        const profileElement = document.createElement('div');
        profileElement.className = 'profile-item';
        profileElement.innerHTML = `
            <div class="profile-info">
                <h4>${profile.name} ${profile.favorite ? '⭐' : ''}</h4>
                <div class="profile-details">
                    ${profile.host} • ${profile.protocol}
                    ${profile.description ? `<br><small>${profile.description}</small>` : ''}
                    ${profile.last_used_at ? `<br><small>Last used: ${new Date(profile.last_used_at).toLocaleDateString()}</small>` : ''}
                    ${profile.usage_count > 0 ? `<br><small>Used ${profile.usage_count} time${profile.usage_count === 1 ? '' : 's'}</small>` : ''}
                </div>
            </div>
            <div class="profile-actions">
                <button class="btn btn-secondary" onclick="toggleFavorite('${profile.id}')">
                    ${profile.favorite ? '★' : '☆'}
                </button>
                <button class="btn btn-secondary" onclick="editProfile('${profile.id}')">
                    Edit
                </button>
                <button class="btn btn-danger" onclick="deleteProfile('${profile.id}')">
                    Delete
                </button>
            </div>
        `;
        profilesList.appendChild(profileElement);
    });
}

function openProfileModal(profileId = null) {
    editingProfileId = profileId;

    if (profileId) {
        // Edit mode
        const profile = currentProfiles.find(p => p.id === profileId);
        if (profile) {
            modalTitle.textContent = 'Edit VPN Profile';
            profileNameInput.value = profile.name;
            // Note: We would need to load full profile data here
            // For now, we'll keep it simple
        }
    } else {
        // Add mode
        modalTitle.textContent = 'Add VPN Profile';
        profileForm.reset();
        profilePortInput.value = '443';
        profileTimeoutInput.value = '30';
    }

    profileModal.classList.add('show');
}

function closeProfileModal() {
    profileModal.classList.remove('show');
    profileForm.reset();
    editingProfileId = null;
}

async function handleProfileSubmit(e) {
    e.preventDefault();

    const profileData = {
        id: editingProfileId || generateId(),
        name: profileNameInput.value.trim(),
        host: profileHostInput.value.trim(),
        port: parseInt(profilePortInput.value),
        protocol: profileProtocolSelect.value,
        account_name: profileAccountInput.value.trim() || '',
        timeout: parseInt(profileTimeoutInput.value),
        options: {}
    };

    setSaveButtonLoading(true);

    try {
        await invoke('save_profile', { profile: profileData });

        if (editingProfileId) {
            showSuccess('Profile updated successfully');
        } else {
            showSuccess('Profile created successfully');
        }

        closeProfileModal();
        await loadProfiles();
        await switchView(currentView); // Refresh current view
    } catch (error) {
        console.error('Failed to save profile:', error);
        showError('Failed to save profile: ' + error);
    } finally {
        setSaveButtonLoading(false);
    }
}

async function deleteProfile(profileId) {
    if (!confirm('Are you sure you want to delete this VPN profile?')) {
        return;
    }

    try {
        await invoke('delete_profile', { profileId });
        showSuccess('Profile deleted successfully');
        await loadProfiles();
        await switchView(currentView); // Refresh current view
    } catch (error) {
        console.error('Failed to delete profile:', error);
        showError('Failed to delete profile: ' + error);
    }
}

async function editProfile(profileId) {
    try {
        const profile = await invoke('get_profile', { profileId });

        modalTitle.textContent = 'Edit VPN Profile';
        profileNameInput.value = profile.name;
        profileHostInput.value = profile.host;
        profilePortInput.value = profile.port;
        // Protocol is stored as an enum, extract the variant name
        profileProtocolSelect.value = profile.protocol;
        profileAccountInput.value = profile.account_name || '';
        profileTimeoutInput.value = profile.timeout;

        openProfileModal(profileId);
    } catch (error) {
        console.error('Failed to load profile for editing:', error);
        showError('Failed to load profile data');
    }
}

async function toggleFavorite(profileId) {
    try {
        await invoke('toggle_profile_favorite', { profileId });
        await loadProfilesForCurrentView();
        showSuccess('Favorite status updated');
    } catch (error) {
        console.error('Failed to toggle favorite:', error);
        showError('Failed to update favorite status');
    }
}

async function switchView(view) {
    currentView = view;

    // Update button states
    document.getElementById('show-favorites-btn').className = view === 'favorites' ? 'btn btn-primary' : 'btn btn-secondary';
    document.getElementById('show-recent-btn').className = view === 'recent' ? 'btn btn-primary' : 'btn btn-secondary';
    document.getElementById('show-all-btn').className = view === 'all' ? 'btn btn-primary' : 'btn btn-secondary';

    await loadProfilesForCurrentView();
}

// Status Management
async function updateConnectionStatus(status = null) {
    try {
        if (!status) {
            status = invoke('get_connection_status');
        }

        currentStatus = status;
        updateStatusDisplay(status);
        updateConnectButton(status);
    } catch (error) {
        console.error('Failed to get connection status:', error);
    }
}

function updateStatusDisplay(status) {
    statusText.textContent = status.status_message;

    statusIndicator.className = 'status-indicator';

    if (status.connected) {
        statusIndicator.classList.add('connected');
        connectionInfo.innerHTML = `
            <strong>Connected to:</strong> ${status.profile_name || 'Unknown'}<br>
            <strong>Status:</strong> ${status.status_message}
        `;
    } else if (status.status_message.includes('Connecting')) {
        statusIndicator.classList.add('connecting');
        connectionInfo.innerHTML = '<strong>Status:</strong> Connecting...';
    } else if (status.status_message.includes('Error') || status.status_message.includes('Failed')) {
        statusIndicator.classList.add('error');
        connectionInfo.innerHTML = `<strong>Error:</strong> ${status.status_message}`;
    } else {
        connectionInfo.innerHTML = '<strong>Status:</strong> Disconnected';
    }
}

function updateConnectButton(status) {
    if (status.connected) {
        connectBtn.innerHTML = `
            <span class="btn-text">Disconnect</span>
            <div class="btn-spinner" id="connect-spinner"></div>
        `;
        connectBtn.className = 'btn btn-danger connect-btn';
        connectBtn.onclick = handleDisconnect;
    } else {
        connectBtn.innerHTML = `
            <span class="btn-text">Connect</span>
            <div class="btn-spinner" id="connect-spinner"></div>
        `;
        connectBtn.className = 'btn btn-primary connect-btn';
        connectBtn.onclick = handleConnect;
    }
}

function startStatusPolling() {
    // Poll status every 5 seconds
    setInterval(async () => {
        try {
            updateConnectionStatus();
        } catch (error) {
            console.error('Status polling failed:', error);
        }
    }, 5000);
}

// Utility Functions
function setConnectButtonLoading(loading) {
    connectBtn.classList.toggle('loading', loading);
    connectBtn.disabled = loading;
}

function setSaveButtonLoading(loading) {
    saveProfileBtn.classList.toggle('loading', loading);
    saveProfileBtn.disabled = loading;
}

function generateId() {
    return Date.now().toString(36) + Math.random().toString(36).substr(2);
}

function showSuccess(message) {
    // Simple notification - in a real app, you'd use a proper notification system
    console.log('SUCCESS:', message);
    alert('✅ ' + message);
}

function showError(message) {
    console.error('ERROR:', message);
    alert('❌ ' + message);
}

function closeStatusModal() {
    statusModal.classList.remove('show');
}

// Initialize the application when the page loads
document.addEventListener('DOMContentLoaded', init);

// Make functions global for onclick handlers
window.editProfile = editProfile;
window.deleteProfile = deleteProfile;
window.toggleFavorite = toggleFavorite;
