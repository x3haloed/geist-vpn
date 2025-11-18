//! VPN Profile management
//!
//! Handles loading, saving, and managing VPN connection profiles.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// VPN connection profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VpnProfile {
    /// Unique identifier for the profile
    pub id: String,

    /// Display name for the profile
    pub name: String,

    /// VPN server hostname or IP address
    pub host: String,

    /// VPN server port (default: 443 for SSL-VPN, 500/4500 for L2TP/IPsec)
    pub port: u16,

    /// VPN protocol to use
    pub protocol: VpnProtocol,

    /// Authentication method
    pub auth: AuthMethod,

    /// Account name/username
    pub account_name: String,

    /// Connection timeout in seconds
    pub timeout: u32,

    /// Additional connection options
    pub options: HashMap<String, String>,
}

/// Supported VPN protocols
#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

impl ProfileManager {
    /// Create a new profile manager
    pub fn new() -> Result<Self> {
        let profiles_dir = Self::get_profiles_dir()?;
        fs::create_dir_all(&profiles_dir)?;

        Ok(Self { profiles_dir })
    }

    /// Get the profiles directory path
    fn get_profiles_dir() -> Result<PathBuf> {
        let mut dir = dirs::data_dir()
            .ok_or_else(|| Error::Other("Could not determine data directory".into()))?;

        dir.push("geist-vpn");
        dir.push("profiles");
        Ok(dir)
    }

    /// Load all profiles
    pub fn load_profiles(&self) -> Result<Vec<VpnProfile>> {
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

        Ok(())
    }

    /// Delete a profile
    pub fn delete_profile(&self, profile_id: &str) -> Result<()> {
        let filename = format!("{}.yaml", profile_id);
        let path = self.profiles_dir.join(filename);

        if path.exists() {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Get a specific profile by ID
    pub fn get_profile(&self, profile_id: &str) -> Result<VpnProfile> {
        let filename = format!("{}.yaml", profile_id);
        let path = self.profiles_dir.join(filename);

        let file = fs::File::open(path)?;
        let profile: VpnProfile = serde_yaml::from_reader(file)?;

        Ok(profile)
    }
}

impl VpnProfile {
    /// Create a new VPN profile
    pub fn new(name: String, host: String, port: u16, protocol: VpnProtocol) -> Self {
        let id = format!("{}_{}_{}", name.to_lowercase().replace(" ", "_"), host, port);

        Self {
            id,
            name,
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
        }
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
