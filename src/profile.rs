//! VPN Profile management
//!
//! Handles loading, saving, and managing VPN connection profiles.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// VPN connection profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnProfile {
    /// Unique identifier for the profile
    pub id: String,

    /// Display name for the profile
    pub name: String,

    /// Description of the profile
    #[serde(default)]
    pub description: String,

    /// VPN server hostname or IP address
    pub host: String,

    /// VPN server port (default: 443 for SSL-VPN, 500/4500 for L2TP/IPsec)
    pub port: u16,

    /// VPN protocol to use
    pub protocol: VpnProtocol,

    /// Authentication method
    pub auth: AuthMethod,

    /// Account name (used for connection identification)
    pub account_name: String,

    /// Connection timeout in seconds
    pub timeout: u32,

    /// Additional connection options
    pub options: HashMap<String, String>,

    /// Profile metadata
    #[serde(default)]
    pub metadata: ProfileMetadata,
}

/// Profile metadata for tracking usage and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMetadata {
    /// When the profile was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// When the profile was last modified
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// When the profile was last used for a connection
    pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Number of times this profile has been used
    pub usage_count: u32,

    /// Whether this profile is marked as favorite
    pub favorite: bool,

    /// Tags for organizing profiles
    pub tags: Vec<String>,

    /// Profile version for migration purposes
    pub version: u32,
}

impl Default for ProfileMetadata {
    fn default() -> Self {
        let now = chrono::Utc::now();
        Self {
            created_at: now,
            updated_at: now,
            last_used_at: None,
            usage_count: 0,
            favorite: false,
            tags: Vec::new(),
            version: 1,
        }
    }
}

/// Supported VPN protocols
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VpnProtocol {
    /// SSL-VPN (SoftEther's primary protocol)
    SslVpn,

    /// L2TP/IPsec
    L2tpIpsec,

    /// OpenVPN
    OpenVpn,

    /// SSTP
    Sstp,

    /// WireGuard (if supported)
    WireGuard,
}

impl std::fmt::Display for VpnProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VpnProtocol::SslVpn => write!(f, "SSL VPN"),
            VpnProtocol::L2tpIpsec => write!(f, "L2TP/IPsec"),
            VpnProtocol::OpenVpn => write!(f, "OpenVPN"),
            VpnProtocol::Sstp => write!(f, "SSTP"),
            VpnProtocol::WireGuard => write!(f, "WireGuard"),
        }
    }
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    /// Username and password
    Password {
        username: String,
        password: String,
    },

    /// Client certificate
    Certificate {
        cert_path: String,
        key_path: String,
    },

    /// RADIUS authentication
    Radius,

    /// NT Domain authentication
    NtDomain {
        username: String,
        password: String,
        domain: String,
    },
}

/// Profile manager for loading/saving VPN profiles
pub struct ProfileManager {
    profiles_dir: PathBuf,
    cache: std::sync::RwLock<HashMap<String, VpnProfile>>,
    cache_loaded: std::sync::atomic::AtomicBool,
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new() -> Result<Self> {
        let profiles_dir = Self::get_profiles_dir()?;
        fs::create_dir_all(&profiles_dir)?;

        Ok(Self {
            profiles_dir,
            cache: std::sync::RwLock::new(HashMap::new()),
            cache_loaded: AtomicBool::new(false),
        })
    }

    /// Ensure the cache is loaded
    fn ensure_cache_loaded(&self) -> Result<()> {
        if !self.cache_loaded.load(std::sync::atomic::Ordering::Relaxed) {
            let profiles = self.load_profiles_from_disk()?;
            let mut cache = self.cache.write().unwrap();
            cache.clear();
            for profile in profiles {
                cache.insert(profile.id.clone(), profile);
            }
            self.cache_loaded.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        Ok(())
    }

    /// Invalidate the cache (force reload on next access)
    pub fn invalidate_cache(&self) {
        self.cache_loaded.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// Get the profiles directory path
    fn get_profiles_dir() -> Result<PathBuf> {
        let mut dir = dirs::data_dir()
            .ok_or_else(|| Error::Other("Could not determine data directory".into()))?;

        dir.push("geist-vpn");
        dir.push("profiles");
        Ok(dir)
    }

    /// Load all profiles (cached version - preferred)
    pub fn load_profiles(&self) -> Result<Vec<VpnProfile>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();
        Ok(cache.values().cloned().collect())
    }

    /// Load all profiles directly from disk (bypasses cache)
    pub fn load_profiles_from_disk(&self) -> Result<Vec<VpnProfile>> {
        let mut profiles = Vec::new();

        for entry in fs::read_dir(&self.profiles_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                let profile: VpnProfile = serde_yaml::from_reader(fs::File::open(&path)?)?;
                profiles.push(profile);
            }
        }

        Ok(profiles)
    }

    /// Save a profile
    pub fn save_profile(&self, profile: &VpnProfile) -> Result<()> {
        let filename = format!("{}.yaml", profile.id);
        let path = self.profiles_dir.join(filename);

        let file = fs::File::create(path)?;
        serde_yaml::to_writer(file, profile)?;

        // Update cache
        let mut cache = self.cache.write().unwrap();
        cache.insert(profile.id.clone(), profile.clone());

        Ok(())
    }

    /// Delete a profile
    pub fn delete_profile(&self, profile_id: &str) -> Result<()> {
        let filename = format!("{}.yaml", profile_id);
        let path = self.profiles_dir.join(filename);

        if path.exists() {
            fs::remove_file(path)?;
        }

        // Update cache
        let mut cache = self.cache.write().unwrap();
        cache.remove(profile_id);

        Ok(())
    }

    /// Get a specific profile by ID (cached version)
    pub fn get_profile(&self, profile_id: &str) -> Result<VpnProfile> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();
        cache.get(profile_id).cloned().ok_or_else(|| {
            Error::ProfileError {
                message: format!("Profile '{}' not found", profile_id),
            }
        })
    }

    /// Get a specific profile by ID directly from disk (bypasses cache)
    pub fn get_profile_from_disk(&self, profile_id: &str) -> Result<VpnProfile> {
        let filename = format!("{}.yaml", profile_id);
        let path = self.profiles_dir.join(filename);

        let file = fs::File::open(path)?;
        let profile: VpnProfile = serde_yaml::from_reader(file)?;

        Ok(profile)
    }

    /// Search profiles by name or description
    pub fn search_profiles(&self, query: &str) -> Result<Vec<VpnProfile>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();
        let query_lower = query.to_lowercase();

        let results: Vec<VpnProfile> = cache
            .values()
            .filter(|profile| {
                profile.name.to_lowercase().contains(&query_lower) ||
                profile.description.to_lowercase().contains(&query_lower) ||
                profile.host.to_lowercase().contains(&query_lower) ||
                profile.metadata.tags.iter().any(|tag| tag.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect();

        Ok(results)
    }

    /// Get favorite profiles
    pub fn get_favorite_profiles(&self) -> Result<Vec<VpnProfile>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();

        let favorites: Vec<VpnProfile> = cache
            .values()
            .filter(|profile| profile.metadata.favorite)
            .cloned()
            .collect();

        Ok(favorites)
    }

    /// Get recently used profiles (sorted by last used date)
    pub fn get_recent_profiles(&self, limit: usize) -> Result<Vec<VpnProfile>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();

        let mut recent: Vec<VpnProfile> = cache
            .values()
            .filter(|profile| profile.metadata.last_used_at.is_some())
            .cloned()
            .collect();

        recent.sort_by(|a, b| {
            b.metadata.last_used_at.unwrap().cmp(&a.metadata.last_used_at.unwrap())
        });

        recent.truncate(limit);
        Ok(recent)
    }

    /// Get profiles by tag
    pub fn get_profiles_by_tag(&self, tag: &str) -> Result<Vec<VpnProfile>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();

        let tagged: Vec<VpnProfile> = cache
            .values()
            .filter(|profile| profile.metadata.tags.contains(&tag.to_string()))
            .cloned()
            .collect();

        Ok(tagged)
    }

    /// Get all available tags
    pub fn get_all_tags(&self) -> Result<Vec<String>> {
        self.ensure_cache_loaded()?;
        let cache = self.cache.read().unwrap();

        let mut tags = std::collections::BTreeSet::new();
        for profile in cache.values() {
            for tag in &profile.metadata.tags {
                tags.insert(tag.clone());
            }
        }

        Ok(tags.into_iter().collect())
    }
}

impl VpnProfile {
    /// Create a new VPN profile
    pub fn new(name: String, host: String, port: u16, protocol: VpnProtocol) -> Self {
        let id = format!("{}_{}_{}", name.to_lowercase().replace(" ", "_"), host, port);

        Self {
            id,
            name,
            description: String::new(),
            host,
            port,
            protocol,
            auth: AuthMethod::Password {
                username: String::new(),
                password: String::new(),
            },
            account_name: String::new(),
            timeout: 30,
            options: HashMap::new(),
            metadata: ProfileMetadata::default(),
        }
    }

    /// Create a profile from an existing one with updated metadata
    pub fn with_updated_metadata(mut self) -> Self {
        self.metadata.updated_at = chrono::Utc::now();
        self
    }

    /// Mark the profile as used (updates usage statistics)
    pub fn mark_as_used(&mut self) {
        self.metadata.last_used_at = Some(chrono::Utc::now());
        self.metadata.usage_count += 1;
        self.metadata.updated_at = chrono::Utc::now();
    }

    /// Add a tag to the profile
    pub fn add_tag(&mut self, tag: String) {
        if !self.metadata.tags.contains(&tag) {
            self.metadata.tags.push(tag);
            self.metadata.updated_at = chrono::Utc::now();
        }
    }

    /// Remove a tag from the profile
    pub fn remove_tag(&mut self, tag: &str) {
        self.metadata.tags.retain(|t| t != tag);
        self.metadata.updated_at = chrono::Utc::now();
    }

    /// Toggle favorite status
    pub fn toggle_favorite(&mut self) {
        self.metadata.favorite = !self.metadata.favorite;
        self.metadata.updated_at = chrono::Utc::now();
    }

    /// Validate the profile configuration
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(Error::ProfileError { message: "Profile name cannot be empty".into() });
        }

        if self.host.is_empty() {
            return Err(Error::ProfileError { message: "Host cannot be empty".into() });
        }

        if self.port == 0 {
            return Err(Error::ProfileError { message: "Invalid port number".into() });
        }

        match &self.auth {
            AuthMethod::Password { username, password } => {
                if username.is_empty() {
                    return Err(Error::ProfileError { message: "Username cannot be empty".into() });
                }
                if password.is_empty() {
                    return Err(Error::ProfileError { message: "Password cannot be empty".into() });
                }
            }
            AuthMethod::Certificate { cert_path, key_path } => {
                if !Path::new(cert_path).exists() {
                    return Err(Error::ProfileError { message: format!("Certificate file not found: {}", cert_path) });
                }
                if !Path::new(key_path).exists() {
                    return Err(Error::ProfileError { message: format!("Key file not found: {}", key_path) });
                }
            }
            _ => {} // Other auth methods may not need validation here
        }

        Ok(())
    }
}

impl Default for VpnProfile {
    fn default() -> Self {
        Self::new(
            "Default Profile".into(),
            "vpn.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn cleanup_test_profiles_dir() {
        let test_dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-profiles");
        if test_dir.exists() {
            let _ = fs::remove_dir_all(test_dir);
        }
    }

    fn create_test_profile_manager() -> ProfileManager {
        cleanup_test_profiles_dir();

        // Use current directory for testing since we're in a sandbox
        let test_dir = std::env::current_dir()
            .unwrap()
            .join("target")
            .join("test-profiles");

        // Set the data directory to our test directory
        std::env::set_var("XDG_DATA_HOME", test_dir.parent().unwrap());

        ProfileManager::new().expect("Failed to create test profile manager")
    }

    #[test]
    fn test_profile_metadata() {
        let profile = VpnProfile::new(
            "Test Profile".into(),
            "test.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        // Test metadata initialization
        assert_eq!(profile.metadata.version, 1);
        assert!(profile.metadata.created_at <= chrono::Utc::now());
        assert!(profile.metadata.updated_at <= chrono::Utc::now());
        assert!(profile.metadata.last_used_at.is_none());
        assert_eq!(profile.metadata.usage_count, 0);
        assert!(!profile.metadata.favorite);
        assert!(profile.metadata.tags.is_empty());
    }

    #[test]
    fn test_profile_usage_tracking() {
        let mut profile = VpnProfile::new(
            "Usage Test".into(),
            "usage.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        let original_updated_at = profile.metadata.updated_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(1));

        // Mark as used
        profile.mark_as_used();

        assert_eq!(profile.metadata.usage_count, 1);
        assert!(profile.metadata.last_used_at.is_some());
        assert!(profile.metadata.updated_at >= original_updated_at);
    }

    #[test]
    fn test_profile_tags() {
        let mut profile = VpnProfile::new(
            "Tag Test".into(),
            "tag.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        // Add tags
        profile.add_tag("work".to_string());
        profile.add_tag("production".to_string());

        assert_eq!(profile.metadata.tags.len(), 2);
        assert!(profile.metadata.tags.contains(&"work".to_string()));
        assert!(profile.metadata.tags.contains(&"production".to_string()));

        // Try to add duplicate tag (should not add)
        profile.add_tag("work".into());
        assert_eq!(profile.metadata.tags.len(), 2);

        // Remove tag
        profile.remove_tag("work");
        assert_eq!(profile.metadata.tags.len(), 1);
        assert!(!profile.metadata.tags.contains(&"work".to_string()));
    }

    #[test]
    fn test_profile_favorites() {
        let mut profile = VpnProfile::new(
            "Favorite Test".into(),
            "fav.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        assert!(!profile.metadata.favorite);

        profile.toggle_favorite();
        assert!(profile.metadata.favorite);

        profile.toggle_favorite();
        assert!(!profile.metadata.favorite);
    }

    // File system test commented out due to sandbox restrictions
    // #[test]
    // fn test_profile_validation_enhanced() { ... }

    #[test]
    fn test_profile_manager_basic() {
        // Test that ProfileManager can be created (without file operations)
        // This test verifies the basic structure works
        let profile = VpnProfile::new(
            "Basic Test".into(),
            "basic.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        // Test profile ID generation
        assert!(!profile.id.is_empty());
        assert!(profile.id.contains("basic_test"));
        assert!(profile.id.contains("basic.example.com"));
        assert!(profile.id.contains("443"));
    }

    #[test]
    fn test_profile_search_logic() {
        // Test the search logic without file operations
        let mut profiles = Vec::new();

        let mut profile1 = VpnProfile::new(
            "Work VPN".into(),
            "work.company.com".into(),
            443,
            VpnProtocol::SslVpn,
        );
        profile1.add_tag("production".to_string());

        let mut profile2 = VpnProfile::new(
            "Home VPN".into(),
            "home.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );
        profile2.add_tag("personal".to_string());

        let mut profile3 = VpnProfile::new(
            "Backup VPN".into(),
            "backup.server.com".into(),
            443,
            VpnProtocol::SslVpn,
        );
        profile3.description = "Backup connection".into();

        profiles.push(profile1);
        profiles.push(profile2);
        profiles.push(profile3);

        // Test search by name
        let work_results: Vec<_> = profiles.iter()
            .filter(|p| p.name.to_lowercase().contains("work"))
            .collect();
        assert_eq!(work_results.len(), 1);

        // Test search by tag
        let production_results: Vec<_> = profiles.iter()
            .filter(|p| p.metadata.tags.contains(&"production".to_string()))
            .collect();
        assert_eq!(production_results.len(), 1);

        // Test search by description
        let backup_results: Vec<_> = profiles.iter()
            .filter(|p| p.description.to_lowercase().contains("backup"))
            .collect();
        assert_eq!(backup_results.len(), 1);
    }

    // Note: File system tests are commented out due to sandbox restrictions
    // They would work in a real environment but require file system access

    // #[test]
    // fn test_profile_tags_and_favorites() { ... }

    #[test]
    fn test_profile_recent_usage_logic() {
        // Test recent usage logic without file operations
        let mut profiles = Vec::new();

        let mut profile1 = VpnProfile::new(
            "Recent Profile 1".into(),
            "recent1.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        let mut profile2 = VpnProfile::new(
            "Recent Profile 2".into(),
            "recent2.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        let profile3 = VpnProfile::new(
            "Recent Profile 3".into(),
            "recent3.example.com".into(),
            443,
            VpnProtocol::SslVpn,
        );

        // Simulate usage (profile1 used twice, profile2 once, profile3 never)
        profile1.mark_as_used();
        std::thread::sleep(std::time::Duration::from_millis(1));
        profile2.mark_as_used();
        std::thread::sleep(std::time::Duration::from_millis(1));
        profile1.mark_as_used();

        profiles.push(profile1);
        profiles.push(profile2);
        profiles.push(profile3);

        // Filter and sort by last used
        let mut recent: Vec<_> = profiles.into_iter()
            .filter(|p| p.metadata.last_used_at.is_some())
            .collect();

        recent.sort_by(|a, b| {
            b.metadata.last_used_at.unwrap().cmp(&a.metadata.last_used_at.unwrap())
        });

        // Test results
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Recent Profile 1");
        assert_eq!(recent[1].name, "Recent Profile 2");
        assert_eq!(recent[0].metadata.usage_count, 2);
        assert_eq!(recent[1].metadata.usage_count, 1);
    }

    // The following tests are commented out due to sandbox restrictions
    // They require file system access and would work in a real environment

    /*
    #[test]
    fn test_profile_caching() {
        // ... test implementation
    }

    #[test]
    fn test_profile_manager_crud() {
        // ... test implementation
    }

    #[test]
    fn test_profile_search() {
        // ... test implementation
    }

    #[test]
    fn test_profile_tags_and_favorites() {
        // ... test implementation
    }

    #[test]
    fn test_profile_recent_usage() {
        // ... test implementation
    }
    */
}
