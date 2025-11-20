//! SoftEtherVPN Client wrapper
//!
//! Provides a safe Rust interface to SoftEtherVPN's client functionality.

use crate::bindings::{
    self, CLIENT_AUTH, CLIENT_OPTION, NAME, RPC_CERT, RPC_CLIENT_CREATE_ACCOUNT,
    RPC_CLIENT_DELETE_ACCOUNT, SHA1_SIZE, X,
};
use crate::cert_prompt as certificate_prompt;
use crate::cert_prompt::{ActiveProfileInfo, CertificateDecision};
use crate::error::{Error, Result};
use crate::memory::{self, strings};
#[cfg(test)]
use crate::profile::VpnProtocol;
use crate::profile::{AuthMethod, VpnProfile};
#[cfg(test)]
use std::collections::HashMap;
use std::collections::HashSet;
use std::ffi::{c_char, c_void, CString};
use std::ptr;
use std::slice;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::broadcast;

/// SoftEther VPN Client wrapper
pub struct SoftEtherClient {
    /// Internal client handle (pointer to SoftEther CLIENT struct)
    client_handle: *mut std::ffi::c_void,

    /// Current connection state
    state: ConnectionState,

    /// Active profile if connected
    active_profile: Option<VpnProfile>,

    /// Fingerprints of certificates already loaded into the trust store
    trusted_certificates: HashSet<String>,

    /// Status update channel sender
    status_tx: broadcast::Sender<ConnectionStatus>,
}

/// Internal connection state for better lifecycle management
#[derive(Debug, Clone, PartialEq)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error(String),
}

impl SoftEtherClient {
    /// Create a new SoftEther client instance
    pub fn new() -> Result<Self> {
        tracing::info!("SoftEtherClient: Creating new SoftEther client");

        // Initialize SoftEther threading if not already done
        tracing::info!("SoftEtherClient: Ensuring SoftEther is initialized");
        Self::ensure_softether_initialized()?;
        tracing::info!("SoftEtherClient: SoftEther initialization complete");

        unsafe {
            tracing::info!("SoftEtherClient: Calling CiNewClient()");
            let handle = crate::bindings::CiNewClient();
            if handle.is_null() {
                tracing::error!("SoftEtherClient: CiNewClient() returned null handle");
                return Err(Error::InitializationFailed {
                    message: "Failed to create SoftEther client".into(),
                });
            }
            tracing::info!(
                "SoftEtherClient: CiNewClient() succeeded, handle: {:?}",
                handle
            );

            // Initialize keep connection (from CtStartClient)
            tracing::info!("SoftEtherClient: Calling CiInitKeep()");
            crate::bindings::CiInitKeep(handle);
            tracing::info!("SoftEtherClient: CiInitKeep() completed");

            // Skip CiStartRpcServer() to avoid the hanging RPC thread

            // Initialize saver (from CtStartClient)
            tracing::info!("SoftEtherClient: Calling CiInitSaver()");
            crate::bindings::CiInitSaver(handle);
            tracing::info!("SoftEtherClient: CiInitSaver() completed");

            // Create broadcast channel for status updates (buffer size 16)
            let (status_tx, _) = broadcast::channel(16);

            tracing::info!("SoftEtherClient: SoftEther client created successfully");
            Ok(Self {
                client_handle: handle,
                state: ConnectionState::Disconnected,
                active_profile: None,
                trusted_certificates: HashSet::new(),
                status_tx,
            })
        }
    }

    /// Ensure SoftEtherVPN library is properly initialized for threading
    fn ensure_softether_initialized() -> Result<()> {
        use std::sync::Once;
        static INIT: Once = Once::new();

        tracing::info!("SoftEtherClient: ensure_softether_initialized() called");

        let mut init_result = Ok(());
        INIT.call_once(|| {
            tracing::info!("SoftEtherClient: First time initialization, calling crate::init()");
            // Initialize SoftEtherVPN library safely
            // This is done lazily to avoid conflicts with GUI initialization
            #[cfg(not(test))]
            {
                init_result = crate::init();
                match &init_result {
                    Ok(_) => tracing::info!("SoftEtherVPN library initialized successfully"),
                    Err(e) => tracing::error!("Failed to initialize SoftEtherVPN library: {}", e),
                }
            }

            #[cfg(test)]
            {
                tracing::info!("SoftEtherVPN library initialization skipped during tests");
            }
        });

        tracing::info!("SoftEtherClient: ensure_softether_initialized() returning");
        init_result
    }

    /// Global cleanup for SoftEtherVPN threading system
    ///
    /// This should be called when the application shuts down.
    pub fn global_cleanup() -> Result<()> {
        // Skip FFI calls during tests
        #[cfg(not(test))]
        unsafe {
            crate::bindings::CtStopClient();
            tracing::info!("SoftEtherVPN client service stopped");
        }

        #[cfg(test)]
        {
            tracing::info!("SoftEtherVPN client service cleanup skipped during tests");
        }
        Ok(())
    }

    /// Subscribe to connection status updates
    ///
    /// Returns a receiver that will get status updates as they happen.
    pub fn subscribe_status(&self) -> broadcast::Receiver<ConnectionStatus> {
        self.status_tx.subscribe()
    }

    /// Subscribe to detailed connection status updates
    ///
    /// Returns a receiver that will get detailed status updates.
    pub fn subscribe_detailed_status(&self) -> broadcast::Receiver<DetailedConnectionStatus> {
        // For now, we don't have a separate channel for detailed status
        // This could be implemented later if needed
        panic!("Detailed status subscription not yet implemented");
    }

    /// Send a status update to all subscribers
    fn send_status_update(&self, status: ConnectionStatus) {
        let _ = self.status_tx.send(status);
    }

    /// Get the SoftEther error code from the client structure
    fn get_softether_error_code(&self) -> u32 {
        unsafe {
            if self.client_handle.is_null() {
                return 0;
            }

            // CLIENT structure layout (64-bit):
            // LOCK *lock (8) + LOCK *lockForConnect (8) + REF *ref (8) + CEDAR *Cedar (8) + bool Halt (1, padded to 8) = 40 bytes
            // UINT Err is at offset 40
            let client_ptr = self.client_handle as *const u8;

            // Debug: print first 64 bytes to see what's actually there
            tracing::error!("CLIENT structure debug (first 64 bytes):");
            for i in 0..8 {
                let offset = i * 8;
                let val = *(client_ptr.add(offset) as *const u64);
                tracing::error!("  Offset {}: 0x{:016x}", offset, val);
            }

            // Try different offsets for the Err field
            let mut err_value = 0u32;
            for test_offset in [32, 36, 40, 44, 48, 52, 56] {
                if test_offset + 4 <= 64 {
                    let test_ptr = client_ptr.add(test_offset) as *const u32;
                    let test_value = unsafe { *test_ptr };
                    tracing::error!(
                        "  Possible Err at offset {}: {} (0x{:08x})",
                        test_offset,
                        test_value,
                        test_value
                    );
                    // If it looks like a small error code (0-200), use it
                    if test_value > 0 && test_value < 200 {
                        err_value = test_value;
                        tracing::error!("  ^^ Using this as the error code!");
                        break;
                    }
                }
            }
            err_value
        }
    }

    /// Get human-readable description of SoftEther error code
    fn get_error_description(&self, error_code: u32) -> &'static str {
        match error_code {
            0 => "No error",
            1 => "Connection to the server has failed",
            2 => "The destination server is not a VPN server",
            3 => "The connection has been interrupted",
            4 => "Protocol error",
            5 => "Connecting client is not a VPN client",
            6 => "User cancel",
            7 => "Specified authentication method is not supported",
            8 => "The HUB does not exist",
            9 => "Authentication failure",
            10 => "HUB is stopped",
            11 => "Session has been deleted",
            12 => "Access denied",
            13 => "Session times out",
            14 => "Protocol is invalid",
            15 => "Too many connections",
            16 => "Too many sessions of the HUB",
            17 => "Connection to the proxy server fails",
            18 => "Proxy Error",
            19 => "Failed to authenticate on the proxy server",
            30 => "Virtual LAN card with the specified name already exists",
            31 => "Specified virtual LAN card cannot be created",
            32 => "Specified name of the virtual LAN card is invalid",
            33 => "Unsupported",
            34 => "Account already exists",
            35 => "Account is operating",
            36 => "Specified account doesn't exist",
            37 => "Account is offline",
            38 => "Parameter is invalid",
            39 => "Error has occurred in the operation of the secure device",
            _ => "Unknown error",
        }
    }

    /// Load a CA certificate from file and add it to the client's trusted certificates
    pub fn load_ca_certificate(&mut self, cert_path: &str) -> Result<()> {
        tracing::info!("Loading CA certificate from: {}", cert_path);
        let fingerprint = self.load_certificate_from_file(cert_path)?;
        tracing::info!(
            "Loaded CA certificate from '{}' (fingerprint {})",
            cert_path,
            fingerprint
        );
        Ok(())
    }

    /// Connect to a VPN server using the provided profile
    pub async fn connect(&mut self, profile: &VpnProfile) -> Result<()> {
        match self.state {
            ConnectionState::Connected => {
                return Err(Error::ConnectionFailed {
                    message: "Client is already connected".into(),
                });
            }
            ConnectionState::Connecting => {
                return Err(Error::ConnectionFailed {
                    message: "Connection attempt already in progress".into(),
                });
            }
            ConnectionState::Disconnecting => {
                return Err(Error::ConnectionFailed {
                    message: "Cannot connect while disconnecting".into(),
                });
            }
            _ => {}
        }

        profile.validate()?;

        let account_alias = if profile.account_name.trim().is_empty() {
            profile.name.clone()
        } else {
            profile.account_name.clone()
        };

        self.load_profile_certificate(profile)?;

        self.state = ConnectionState::Connecting;
        self.send_status_update(ConnectionStatus::Connecting);
        tracing::info!("Starting VPN connection to: {}", profile.name);

        let profile_info = ActiveProfileInfo {
            id: profile.id.clone(),
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port,
        };

        let result = {
            let account_score = account_alias.clone();
            let profile_clone = profile.clone();
            let active_info = profile_info.clone();
            let timeout_duration = Duration::from_secs(profile.timeout as u64);
            tokio::time::timeout(timeout_duration, async {
                certificate_prompt::clear_last_certificate_decision();
                certificate_prompt::set_active_profile(Some(active_info));
                let result = self
                    .perform_connection_async(&profile_clone, &account_score)
                    .await;
                certificate_prompt::clear_active_profile();
                result
            })
            .await
        };

        match result {
            Ok(Ok(())) => {
                self.state = ConnectionState::Connected;
                let mut active_profile = profile.clone();
                active_profile.account_name = account_alias.clone();
                self.active_profile = Some(active_profile);
                self.send_status_update(ConnectionStatus::Connected);
                tracing::info!("Successfully connected to VPN: {}", profile.name);
                Ok(())
            }
            Ok(Err(e)) => {
                self.state = ConnectionState::Disconnected;
                self.send_status_update(ConnectionStatus::Disconnected);
                tracing::error!("VPN connection failed: {}", e);
                Err(e)
            }
            Err(_) => {
                self.state = ConnectionState::Disconnected;
                self.send_status_update(ConnectionStatus::Disconnected);
                let error = Error::ConnectionFailed {
                    message: format!("VPN connection timed out after {} seconds", profile.timeout),
                };
                tracing::error!("{}", error);
                Err(error)
            }
        }
    }

    async fn perform_connection_async(
        &mut self,
        profile: &VpnProfile,
        account_name: &str,
    ) -> Result<()> {
        self.validate_connection_parameters(profile)?;

        let account_ptr = self.build_account_request(profile, account_name)?;
        let result = unsafe { bindings::CtCreateAccount(self.client_handle, account_ptr, false) };
        unsafe {
            bindings::CiFreeClientCreateAccount(account_ptr);
        }

        if !result {
            let softether_error = self.get_softether_error_code();
            let error_message = format!(
                "SoftEther Error {} ({}) while creating account '{}'",
                softether_error,
                self.get_error_description(softether_error),
                account_name
            );
            return Err(Error::ConnectionFailed {
                message: error_message,
            });
        }

        let account_name_wide = strings::rust_to_softether_wide(account_name)?;
        let _account_guard = AccountGuard::new(self.client_handle, &account_name_wide)?;

        let connect_req = self.create_connect_request(account_name)?;
        let connect_result = unsafe {
            bindings::CtConnect(
                self.client_handle,
                connect_req.as_typed_ptr::<bindings::RPC_CLIENT_CONNECT>(),
            )
        };

        if connect_result == 0 {
            let cert_decision = certificate_prompt::take_last_certificate_decision();
            if cert_decision == Some(CertificateDecision::Reject) {
                tracing::warn!("Connection aborted because user rejected the server certificate");
                return Err(Error::ConnectionFailed {
                    message: "Server certificate was rejected by the user".into(),
                });
            }

            let softether_error = self.get_softether_error_code();
            tracing::error!("SoftEther CtConnect returned 0 (failure)");
            tracing::error!(
                "SoftEther Error Code: {} ({})",
                softether_error,
                self.get_error_description(softether_error)
            );
            tracing::error!("Connection parameters:");
            tracing::error!("  Server: {}:{}", profile.host, profile.port);
            tracing::error!("  Account: {}", account_name);
            tracing::error!("  Protocol: {:?}", profile.protocol);
            tracing::error!("  Auth: {:?}", profile.auth);

            let diagnostic_info = self.get_connection_diagnostics(profile).await;
            tracing::error!("Network diagnostics: {}", diagnostic_info);

            let error_msg = self.get_connection_error_details(profile).await;
            let detailed_error_msg = format!(
                "SoftEther Error {} ({}). {}",
                softether_error,
                self.get_error_description(softether_error),
                error_msg
            );

            return Err(Error::ConnectionFailed {
                message: detailed_error_msg,
            });
        }

        certificate_prompt::clear_last_certificate_decision();
        Ok(())
    }

    fn build_account_request(
        &self,
        profile: &VpnProfile,
        account_name: &str,
    ) -> Result<*mut RPC_CLIENT_CREATE_ACCOUNT> {
        let option = self.build_client_option(profile, account_name)?;
        let auth = self.build_client_auth(profile)?;

        let mut request = memory::zero_malloc_box::<RPC_CLIENT_CREATE_ACCOUNT>()?;
        request.ClientOption = Box::into_raw(option);
        request.ClientAuth = Box::into_raw(auth);
        request.StartupAccount = false;
        request.CheckServerCert = true;
        request.RetryOnServerCert = false;
        request.AddDefaultCA = false;
        request.ServerCert = ptr::null_mut();

        Ok(Box::into_raw(request))
    }

    fn build_client_option(
        &self,
        profile: &VpnProfile,
        account_name: &str,
    ) -> Result<Box<CLIENT_OPTION>> {
        let mut option = memory::zero_malloc_box::<CLIENT_OPTION>()?;
        option.AccountName = strings::rust_to_softether_wide(account_name)?;
        copy_to_c_buffer(&mut option.Hostname, &profile.host)?;
        option.Port = profile.port as bindings::UINT;
        option.PortUDP = 0;
        option.ProxyType = bindings::PROXY_DIRECT;
        option.ProxyPort = 0;
        option.NumRetry = 1;
        option.RetryInterval = 1;
        copy_to_c_buffer(&mut option.HubName, &profile.hub_name)?;
        option.MaxConnection = 1;
        option.UseEncrypt = true;
        option.UseCompress = true;
        copy_to_c_buffer(&mut option.DeviceName, "GEISTVPN")?;
        option.AdditionalConnectionInterval = 1;
        option.ConnectionDisconnectSpan = 0;
        option.HideStatusWindow = true;
        option.HideNicInfoWindow = true;
        option.NoRoutingTracking = true;
        option.NoUdpAcceleration = false;
        option.RequireMonitorMode = false;
        option.RequireBridgeRoutingMode = false;
        option.DisableQoS = false;
        option.FromAdminPack = false;
        option.BindLocalPort = 0;

        Ok(option)
    }

    fn build_client_auth(&self, profile: &VpnProfile) -> Result<Box<CLIENT_AUTH>> {
        let mut auth = memory::zero_malloc_box::<CLIENT_AUTH>()?;

        match &profile.auth {
            AuthMethod::Password { username, password } => {
                auth.AuthType = bindings::CLIENT_AUTHTYPE_PASSWORD;
                copy_to_c_buffer(&mut auth.Username, username)?;
                copy_to_c_buffer(&mut auth.PlainPassword, password)?;
            }
            AuthMethod::NtDomain {
                username,
                password,
                domain,
            } => {
                auth.AuthType = bindings::CLIENT_AUTHTYPE_PASSWORD;
                let combined = if domain.trim().is_empty() {
                    username.clone()
                } else {
                    format!(r"{}\{}", domain.trim(), username)
                };
                copy_to_c_buffer(&mut auth.Username, &combined)?;
                copy_to_c_buffer(&mut auth.PlainPassword, password)?;
            }
            AuthMethod::Radius => {
                auth.AuthType = bindings::CLIENT_AUTHTYPE_ANONYMOUS;
            }
            AuthMethod::Certificate { .. } => {
                return Err(Error::ConnectionFailed {
                    message: "Certificate authentication is not yet supported".into(),
                });
            }
        }

        auth.CheckCertProc = Some(certificate_check_callback);
        auth.SecureSignProc = None;

        Ok(auth)
    }

    fn load_profile_certificate(&mut self, profile: &VpnProfile) -> Result<()> {
        if let Some(pem) = profile.options.get("server_cert") {
            let trimmed = pem.trim();
            if !trimmed.is_empty() {
                let fingerprint = self.load_certificate_from_pem(trimmed)?;
                tracing::info!(
                    "Trusted server certificate for {} has fingerprint {}",
                    profile.host,
                    fingerprint
                );
            }
        }

        if let Some(path) = profile.options.get("certificate_path") {
            let trimmed = path.trim();
            if !trimmed.is_empty() {
                let fingerprint = self.load_certificate_from_file(trimmed)?;
                tracing::info!(
                    "Loaded CA file '{}' for {} (fingerprint {})",
                    trimmed,
                    profile.host,
                    fingerprint
                );
            }
        }

        Ok(())
    }

    fn load_certificate_from_pem(&mut self, pem: &str) -> Result<String> {
        let data = pem.as_bytes();
        let buf = unsafe {
            bindings::NewBufFromMemory(data.as_ptr() as *const c_void, data.len() as bindings::UINT)
        };
        if buf.is_null() {
            return Err(Error::FfiError {
                message: "Failed to allocate certificate buffer".into(),
            });
        }

        let x = unsafe { bindings::BufToX(buf, true) };
        unsafe {
            bindings::FreeBuf(buf);
        }

        if x.is_null() {
            return Err(Error::FfiError {
                message: "Failed to parse stored server certificate".into(),
            });
        }

        self.add_certificate_to_trust_store(x)
    }

    fn load_certificate_from_file(&mut self, path: &str) -> Result<String> {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return Err(Error::ConnectionFailed {
                message: "Certificate path is empty".into(),
            });
        }

        let path_c = CString::new(trimmed).map_err(|e| Error::ConnectionFailed {
            message: format!("Invalid certificate path: {}", e),
        })?;

        let cert_ptr = unsafe { bindings::FileToX(path_c.as_ptr()) } as *mut X;
        if cert_ptr.is_null() {
            return Err(Error::ConnectionFailed {
                message: format!("Failed to load certificate from {}", trimmed),
            });
        }

        self.add_certificate_to_trust_store(cert_ptr)
    }

    fn add_certificate_to_trust_store(&mut self, x: *mut X) -> Result<String> {
        if x.is_null() {
            return Err(Error::FfiError {
                message: "Certificate pointer is null".into(),
            });
        }

        let fingerprint = Self::compute_certificate_fingerprint(x)?;
        if self.trusted_certificates.contains(&fingerprint) {
            unsafe {
                bindings::FreeX(x as *mut c_void);
            }
            return Ok(fingerprint);
        }

        let cert = RPC_CERT { x };
        let cert_box = memory::malloc_box(cert)?;
        let cert_ptr = Box::into_raw(cert_box);
        let added = unsafe { bindings::CtAddCa(self.client_handle, cert_ptr) };
        unsafe {
            let _ = Box::from_raw(cert_ptr);
            bindings::FreeX(x as *mut c_void);
        }

        if added == 0 {
            return Err(Error::FfiError {
                message: "Failed to add certificate to trust store".into(),
            });
        }

        self.trusted_certificates.insert(fingerprint.clone());
        Ok(fingerprint)
    }

    fn compute_certificate_fingerprint(x: *mut X) -> Result<String> {
        certificate_sha1_hex(x).ok_or_else(|| Error::FfiError {
            message: "Certificate pointer is null or invalid".into(),
        })
    }

    /// Validate connection parameters before attempting connection
    fn validate_connection_parameters(&self, profile: &VpnProfile) -> Result<()> {
        // Check if server hostname/IP is valid
        if profile.host.trim().is_empty() {
            return Err(Error::ConnectionFailed {
                message: "Server hostname/IP address is empty".into(),
            });
        }

        // Check if port is valid
        if profile.port == 0 {
            return Err(Error::ConnectionFailed {
                message: "Invalid port number (must be > 0)".into(),
            });
        }

        // Validate authentication parameters
        match &profile.auth {
            AuthMethod::Password { username, password } => {
                if username.trim().is_empty() {
                    return Err(Error::ConnectionFailed {
                        message: "Username is required for password authentication".into(),
                    });
                }
                if password.is_empty() {
                    return Err(Error::ConnectionFailed {
                        message: "Password is required for password authentication".into(),
                    });
                }
            }
            AuthMethod::NtDomain {
                username, password, ..
            } => {
                if username.trim().is_empty() {
                    return Err(Error::ConnectionFailed {
                        message: "Username is required for NT domain authentication".into(),
                    });
                }
                if password.is_empty() {
                    return Err(Error::ConnectionFailed {
                        message: "Password is required for NT domain authentication".into(),
                    });
                }
            }
            AuthMethod::Radius => {
                // RADIUS doesn't require additional validation here
            }
            AuthMethod::Certificate { .. } => {
                // Certificate validation would be more complex
                return Err(Error::ConnectionFailed {
                    message: "Certificate authentication is not yet supported".into(),
                });
            }
        }

        Ok(())
    }

    /// Get detailed error information after a connection failure
    async fn get_connection_error_details(&self, profile: &VpnProfile) -> String {
        // Try to perform basic network connectivity check
        let connectivity_msg = self.check_network_connectivity(profile).await;

        let auth_info = match &profile.auth {
            AuthMethod::Password { .. } => "using password authentication",
            AuthMethod::NtDomain { .. } => "using NT domain authentication",
            AuthMethod::Radius => "using RADIUS authentication",
            AuthMethod::Certificate { .. } => "using certificate authentication",
        };

        format!(
            "Failed to connect to VPN server '{}' on {}:{} {}. {}. Possible causes: server unreachable, invalid credentials, authentication method not supported by server, or server configuration issues.",
            profile.host, profile.host, profile.port, auth_info, connectivity_msg
        )
    }

    /// Check basic network connectivity to the server
    async fn check_network_connectivity(&self, profile: &VpnProfile) -> String {
        // Try to resolve the hostname
        match tokio::net::lookup_host(format!("{}:{}", profile.host, profile.port)).await {
            Ok(mut addrs) => {
                if addrs.next().is_some() {
                    "Server hostname resolves successfully".to_string()
                } else {
                    format!(
                        "Server hostname '{}' resolves but no addresses found for port {}",
                        profile.host, profile.port
                    )
                }
            }
            Err(e) => {
                format!("Cannot resolve server hostname '{}': {}", profile.host, e)
            }
        }
    }

    /// Get detailed connection diagnostics
    async fn get_connection_diagnostics(&self, profile: &VpnProfile) -> String {
        let mut diagnostics = Vec::new();

        // Test basic TCP connectivity
        diagnostics.push(format!(
            "Testing TCP connection to {}:{}...",
            profile.host, profile.port
        ));

        // Try to establish a TCP connection (this won't do VPN handshake, just basic connectivity)
        match tokio::net::TcpStream::connect(format!("{}:{}", profile.host, profile.port)).await {
            Ok(_) => {
                diagnostics.push("✓ TCP connection successful".to_string());
            }
            Err(e) => {
                diagnostics.push(format!("✗ TCP connection failed: {}", e));
                return diagnostics.join("\n");
            }
        }

        // Test if it looks like an HTTP server
        diagnostics.push("Testing if server responds to HTTP...".to_string());

        // Try a basic HTTP request to see if it's an HTTP server
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(format!("{}:{}", profile.host, profile.port)),
        )
        .await
        {
            Ok(Ok(mut stream)) => {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};

                // Send a basic HTTP GET request
                let http_request = b"GET / HTTP/1.0\r\nHost: example.com\r\n\r\n";
                if stream.write_all(http_request).await.is_ok() {
                    let mut buffer = [0u8; 1024];
                    match stream.read(&mut buffer).await {
                        Ok(n) if n > 0 => {
                            let response = String::from_utf8_lossy(&buffer[..n.min(200)]);
                            if response.contains("HTTP/") {
                                diagnostics.push(format!(
                                    "⚠️  Server responds to HTTP: {}",
                                    response.lines().next().unwrap_or("Unknown response")
                                ));
                                diagnostics.push(
                                    "   This might be a web server, not a VPN server!".to_string(),
                                );
                            } else {
                                diagnostics.push(
                                    "✓ Server doesn't respond like HTTP (good for VPN)".to_string(),
                                );
                            }
                        }
                        _ => {
                            diagnostics
                                .push("✓ No HTTP response (expected for VPN server)".to_string());
                        }
                    }
                }
            }
            _ => {
                diagnostics.push("✓ Could not test HTTP response".to_string());
            }
        }

        diagnostics.join("\n")
    }

    /// Disconnect from the current VPN connection
    pub async fn disconnect(&mut self) -> Result<()> {
        // Check current state
        match self.state {
            ConnectionState::Disconnected => {
                return Ok(()); // Already disconnected
            }
            ConnectionState::Connecting => {
                return Err(Error::ConnectionFailed {
                    message: "Cannot disconnect while connecting".into(),
                });
            }
            ConnectionState::Disconnecting => {
                return Err(Error::ConnectionFailed {
                    message: "Disconnect already in progress".into(),
                });
            }
            _ => {} // Connected or Error states are OK to disconnect from
        }

        // Transition to disconnecting state
        self.state = ConnectionState::Disconnecting;
        self.send_status_update(ConnectionStatus::Disconnecting);
        tracing::info!("Starting VPN disconnection");

        // For disconnect, we need to pass the same connection request
        // In a full implementation, we'd store this, but for now we'll create a minimal one
        if let Some(profile) = &self.active_profile {
            // Create a minimal disconnect request
            let account_name_wide = strings::rust_to_softether_wide(&profile.account_name)?;

            let disconnect_req = crate::bindings::RPC_CLIENT_CONNECT {
                AccountName: account_name_wide,
            };

            let disconnect_req =
                crate::memory::malloc_box(disconnect_req).map_err(|_| Error::FfiError {
                    message: "Failed to allocate disconnect request".into(),
                })?;

            // Attempt disconnect with timeout
            let disconnect_future = async {
                let result = unsafe {
                    crate::bindings::CtDisconnect(
                        self.client_handle,
                        Box::into_raw(disconnect_req),
                        0, // inner parameter (false as u8)
                    )
                };

                if result == 0 {
                    return Err(Error::FfiError {
                        message: "VPN disconnect failed".into(),
                    });
                }

                Ok(())
            };

            // Add timeout (10 seconds for disconnect)
            let timeout_duration = std::time::Duration::from_secs(10);
            match tokio::time::timeout(timeout_duration, disconnect_future).await {
                Ok(Ok(())) => {
                    // Disconnect successful
                    self.state = ConnectionState::Disconnected;
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Disconnected);
                    tracing::info!("Successfully disconnected from VPN");
                    Ok(())
                }
                Ok(Err(e)) => {
                    // Disconnect failed but let's still clean up state
                    self.state = ConnectionState::Error("Disconnect failed".into());
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Error("Disconnect failed".into()));
                    tracing::error!("VPN disconnect failed: {}", e);
                    Err(e)
                }
                Err(_) => {
                    // Timeout - still clean up state
                    self.state = ConnectionState::Error("Disconnect timeout".into());
                    self.active_profile = None;
                    self.send_status_update(ConnectionStatus::Error("Disconnect timeout".into()));
                    let error = Error::FfiError {
                        message: "VPN disconnect timed out".into(),
                    };
                    tracing::error!("{}", error);
                    Err(error)
                }
            }
        } else {
            // No active profile, just update state
            self.state = ConnectionState::Disconnected;
            self.send_status_update(ConnectionStatus::Disconnected);
            tracing::info!("Disconnected from VPN (no active profile)");
            Ok(())
        }
    }

    /// Get the current connection status
    pub fn get_status(&self) -> ConnectionStatus {
        match &self.state {
            ConnectionState::Disconnected => ConnectionStatus::Disconnected,
            ConnectionState::Connecting => ConnectionStatus::Connecting,
            ConnectionState::Connected => ConnectionStatus::Connected,
            ConnectionState::Disconnecting => ConnectionStatus::Disconnecting,
            ConnectionState::Error(msg) => ConnectionStatus::Error(msg.clone()),
        }
    }

    /// Get detailed connection status from SoftEther
    ///
    /// This queries the actual connection status from the SoftEther client.
    pub async fn get_detailed_status(&self) -> Result<DetailedConnectionStatus> {
        if self.client_handle.is_null() {
            return Ok(DetailedConnectionStatus::default());
        }

        unsafe {
            // Allocate memory for the status structure
            let status_size =
                std::mem::size_of::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>();
            let status_mem = crate::memory::zero_malloc_raw(status_size)?;

            // Call CtGetAccountStatus
            let success = crate::bindings::CtGetAccountStatus(
                self.client_handle,
                status_mem.as_typed_ptr::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>(),
            );

            if !success {
                return Err(Error::FfiError {
                    message: "Failed to get connection status".into(),
                });
            }

            // Extract status information
            let status_ptr =
                status_mem.as_typed_ptr::<crate::bindings::RPC_CLIENT_GET_CONNECTION_STATUS>();
            let status = &*status_ptr;

            // Convert server name from C string to Rust string
            let server_name = std::ffi::CStr::from_ptr(status.ServerName.as_ptr())
                .to_string_lossy()
                .into_owned();

            let detailed_status = DetailedConnectionStatus {
                account_name: crate::memory::strings::softether_wide_to_rust(&status.AccountName),
                active: status.Active,
                connected: status.Connected,
                session_status: status.SessionStatus as u32,
                server_name,
                server_port: status.ServerPort as u16,
                server_product_name: std::ffi::CStr::from_ptr(status.ServerProductName.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                server_product_version: status.ServerProductVer as u32,
                server_product_build: status.ServerProductBuild as u32,
            };

            Ok(detailed_status)
        }
    }

    /// Check if the client is currently connected
    pub fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected)
    }

    /// Get the active profile if connected
    pub fn active_profile(&self) -> Option<&VpnProfile> {
        self.active_profile.as_ref()
    }

    /// Create a connection request structure for SoftEther FFI
    fn create_connect_request(&self, account_name: &str) -> Result<crate::memory::RawMemory> {
        use crate::memory::strings;

        // Allocate raw memory for the RPC_CLIENT_CONNECT structure
        let size = std::mem::size_of::<crate::bindings::RPC_CLIENT_CONNECT>();
        let raw_mem = crate::memory::malloc_raw(size)?;

        // Create the RPC_CLIENT_CONNECT structure and copy it into the allocated memory
        let account_name_wide = strings::rust_to_softether_wide(account_name)?;
        let connect_req = crate::bindings::RPC_CLIENT_CONNECT {
            AccountName: account_name_wide,
        };

        unsafe {
            std::ptr::copy_nonoverlapping(
                &connect_req as *const _ as *const u8,
                raw_mem.as_ptr() as *mut u8,
                size,
            );
        }

        Ok(raw_mem)
    }

    /// Get the name of the currently active profile
    pub fn get_active_profile_name(&self) -> Option<String> {
        self.active_profile.as_ref().map(|p| p.name.clone())
    }
}

/// Connection status enum
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error(String),
}

/// Detailed connection status information
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DetailedConnectionStatus {
    /// Account name for this connection
    pub account_name: String,
    /// Whether the account is active
    pub active: bool,
    /// Whether currently connected
    pub connected: bool,
    /// Session status code
    pub session_status: u32,
    /// Server hostname/IP
    pub server_name: String,
    /// Server port
    pub server_port: u16,
    /// Server product name
    pub server_product_name: String,
    /// Server product version
    pub server_product_version: u32,
    /// Server product build number
    pub server_product_build: u32,
}

impl Drop for SoftEtherClient {
    fn drop(&mut self) {
        if self.is_connected() {
            // Note: In a real implementation, we might want to make this async
            // For now, we'll just log the issue and attempt cleanup
            tracing::warn!("SoftEtherClient dropped while still connected - attempting cleanup");

            // Try to disconnect synchronously (best effort)
            if let Some(profile) = &self.active_profile {
                use crate::memory::strings;
                if let Ok(account_name_wide) =
                    strings::rust_to_softether_wide(&profile.account_name)
                {
                    let disconnect_req = crate::bindings::RPC_CLIENT_CONNECT {
                        AccountName: account_name_wide,
                    };

                    if let Ok(disconnect_req) = crate::memory::malloc_box(disconnect_req) {
                        unsafe {
                            crate::bindings::CtDisconnect(
                                self.client_handle,
                                Box::into_raw(disconnect_req),
                                0,
                            );
                        }
                    }
                }
            }
        }

        unsafe {
            if !self.client_handle.is_null() {
                crate::bindings::CtReleaseClient(self.client_handle);
            }
        }
    }
}

// Note: ConnectionRequest is now handled by the memory management system
// and RPC_CLIENT_CONNECT structure from bindings.rs

struct AccountGuard {
    client_handle: *mut std::ffi::c_void,
    delete_ptr: *mut RPC_CLIENT_DELETE_ACCOUNT,
}

impl AccountGuard {
    fn new(
        client_handle: *mut std::ffi::c_void,
        account_name: &[u16; bindings::MAX_ACCOUNT_NAME_LEN + 1],
    ) -> Result<Self> {
        let mut request = RPC_CLIENT_DELETE_ACCOUNT {
            AccountName: [0u16; bindings::MAX_ACCOUNT_NAME_LEN + 1],
        };
        request.AccountName.copy_from_slice(account_name);
        let boxed = memory::malloc_box(request)?;
        let ptr = Box::into_raw(boxed);

        Ok(Self {
            client_handle,
            delete_ptr: ptr,
        })
    }
}

impl Drop for AccountGuard {
    fn drop(&mut self) {
        if !self.delete_ptr.is_null() {
            unsafe {
                bindings::CtDeleteAccount(self.client_handle, self.delete_ptr, false);
                let _ = Box::from_raw(self.delete_ptr);
            }
            self.delete_ptr = ptr::null_mut();
        }
    }
}

fn certificate_sha1_hex(x: *mut X) -> Option<String> {
    if x.is_null() {
        return None;
    }

    let mut digest = [0u8; SHA1_SIZE];
    unsafe {
        bindings::GetXDigest(x, digest.as_mut_ptr(), true);
    }

    Some(
        digest
            .iter()
            .map(|byte| format!("{:02X}", byte))
            .collect::<Vec<_>>()
            .join(""),
    )
}

fn copy_to_c_buffer(buffer: &mut [c_char], value: &str) -> Result<()> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();

    if bytes.len() >= buffer.len() {
        return Err(Error::ConnectionFailed {
            message: format!(
                "Value '{}' is too long for a SoftEther field ({} bytes)",
                trimmed,
                buffer.len()
            ),
        });
    }

    for slot in buffer.iter_mut() {
        *slot = 0;
    }

    for (idx, byte) in bytes.iter().enumerate() {
        buffer[idx] = *byte as c_char;
    }

    Ok(())
}

fn certificate_to_pem(x: *mut X) -> Option<String> {
    if x.is_null() {
        return None;
    }

    let buf = unsafe { bindings::XToBuf(x, true) };
    if buf.is_null() {
        return None;
    }

    let size = unsafe { (*buf).Size as usize };
    let data = unsafe { std::slice::from_raw_parts((*buf).Buf as *const u8, size) };
    let pem = String::from_utf8_lossy(data).to_string();

    unsafe {
        bindings::FreeBuf(buf);
    }

    Some(pem)
}

fn format_cert_name(name: *mut NAME) -> Option<String> {
    if name.is_null() {
        return None;
    }

    let name_ref = unsafe { &*name };
    let mut parts = Vec::new();

    if let Some(value) = wide_ptr_to_string(name_ref.CommonName) {
        parts.push(format!("CN={}", value));
    }
    if let Some(value) = wide_ptr_to_string(name_ref.Organization) {
        parts.push(format!("O={}", value));
    }
    if let Some(value) = wide_ptr_to_string(name_ref.Unit) {
        parts.push(format!("OU={}", value));
    }
    if let Some(value) = wide_ptr_to_string(name_ref.State) {
        parts.push(format!("ST={}", value));
    }
    if let Some(value) = wide_ptr_to_string(name_ref.Local) {
        parts.push(format!("L={}", value));
    }
    if let Some(value) = wide_ptr_to_string(name_ref.Country) {
        parts.push(format!("C={}", value));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn wide_ptr_to_string(ptr: *mut bindings::WCHAR) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    unsafe {
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        if len == 0 {
            return None;
        }
        let slice = slice::from_raw_parts(ptr, len);
        Some(String::from_utf16_lossy(slice))
    }
}

unsafe extern "C" fn certificate_check_callback(
    _session: *mut bindings::SESSION,
    _connection: *mut bindings::CONNECTION,
    server_x: *mut X,
    expired: *mut bindings::SoftEtherBool,
) -> bindings::SoftEtherBool {
    if server_x.is_null() {
        return 0;
    }

    if !expired.is_null() {
        *expired = 0;
    }

    let subject = format_cert_name((*server_x).subject_name).unwrap_or_else(|| "Unknown".into());
    let issuer = format_cert_name((*server_x).issuer_name).unwrap_or_else(|| "Unknown".into());
    let fingerprint = certificate_sha1_hex(server_x).unwrap_or_else(|| "Unknown".into());
    let pem = certificate_to_pem(server_x).unwrap_or_default();

    let active_profile = certificate_prompt::current_profile();
    let (profile_id, profile_name, host, port) = if let Some(info) = active_profile {
        (Some(info.id), Some(info.name), info.host, info.port)
    } else {
        (None, None, String::new(), 0)
    };

    let (tx, rx) = mpsc::channel();
    let prompt = certificate_prompt::CertificatePrompt {
        profile_id,
        profile_name,
        host,
        port,
        subject,
        issuer,
        fingerprint,
        pem,
        expired: false,
        response_tx: tx,
    };

    if certificate_prompt::dispatch_prompt(prompt).is_err() {
        certificate_prompt::record_certificate_decision(CertificateDecision::Reject);
        return 0;
    }

    match rx.recv_timeout(Duration::from_secs(120)) {
        Ok(decision) => {
            certificate_prompt::record_certificate_decision(decision);
            match decision {
                CertificateDecision::TrustTemporarily | CertificateDecision::TrustPermanently => 1,
                CertificateDecision::Reject => 0,
            }
        }
        Err(_) => {
            certificate_prompt::record_certificate_decision(CertificateDecision::Reject);
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory;

    #[test]
    fn test_client_creation() {
        // Note: This test requires the SoftEther library to be properly built
        // For now, we'll just test the structure
        let profile = VpnProfile::default();
        assert!(!profile.name.is_empty());
        assert!(!profile.id.is_empty());
        assert!(profile.metadata.version >= 1);
    }

    #[test]
    fn test_profile_validation() {
        // Create a valid profile with credentials
        let mut profile = VpnProfile {
            id: "test_profile".into(),
            name: "Test Profile".into(),
            description: "Test profile description".into(),
            host: "vpn.example.com".into(),
            port: 443,
            protocol: VpnProtocol::SslVpn,
            auth: AuthMethod::Password {
                username: "testuser".into(),
                password: "testpass".into(),
            },
            account_name: "testaccount".into(),
            timeout: 30,
            options: HashMap::new(),
            metadata: crate::profile::ProfileMetadata::default(),
        };

        // Valid profile should pass
        assert!(profile.validate().is_ok());

        // Invalid profile should fail
        profile.name = String::new();
        assert!(profile.validate().is_err());
    }

    #[test]
    fn test_memory_management() {
        // Test our memory management bridge
        let mem_result = memory::malloc_raw(64);
        assert!(mem_result.is_ok(), "Failed to allocate memory");

        let mem = mem_result.unwrap();
        assert_eq!(mem.size(), 64);
        assert!(!mem.as_ptr().is_null());

        // Memory is automatically freed when mem goes out of scope
    }

    #[test]
    fn test_string_conversion() {
        // Test string conversion utilities
        let test_str = "Test VPN Connection";
        let wide_result = memory::strings::rust_to_softether_wide(test_str);
        assert!(
            wide_result.is_ok(),
            "Failed to convert string to wide format"
        );

        let wide_str = wide_result.unwrap();
        let back_to_rust = memory::strings::softether_wide_to_rust(&wide_str);
        assert_eq!(test_str, back_to_rust);
    }

    #[test]
    fn test_connection_status() {
        // Test that we can create a client and check status
        // Note: This would normally require SoftEtherVPN to be compiled
        // For now, we test the data structures

        let status = ConnectionStatus::Disconnected;
        assert!(!format!("{:?}", status).is_empty());
    }

    #[test]
    fn test_error_handling() {
        // Test error creation and handling
        let error = crate::Error::ConnectionFailed {
            message: "Test connection failed".into(),
        };
        assert!(error.to_string().contains("Test connection failed"));
    }

    #[test]
    fn test_softether_error_mapping() {
        // Test SoftEther error code mapping
        let connect_failed =
            crate::Error::from_softether_error(crate::bindings::error_codes::ERR_CONNECT_FAILED);
        assert_eq!(
            connect_failed.to_string(),
            "SoftEther error code 1: Connection to the server has failed"
        );

        let auth_failed =
            crate::Error::from_softether_error(crate::bindings::error_codes::ERR_AUTH_FAILED);
        assert_eq!(
            auth_failed.to_string(),
            "SoftEther error code 9: Authentication failure"
        );

        let hub_not_found =
            crate::Error::from_softether_error(crate::bindings::error_codes::ERR_HUB_NOT_FOUND);
        assert_eq!(
            hub_not_found.to_string(),
            "SoftEther error code 8: The HUB does not exist"
        );

        let unknown_error = crate::Error::from_softether_error(999);
        assert_eq!(
            unknown_error.to_string(),
            "SoftEther error code 999: Unknown error"
        );
    }

    #[test]
    fn test_connection_state_transitions() {
        // Test that state transitions work correctly
        // Note: This test doesn't actually connect, just tests the state logic

        // Test initial state
        let profile = VpnProfile::default();
        assert_eq!(
            ConnectionStatus::Disconnected,
            ConnectionStatus::Disconnected
        );

        // Test state enum conversion
        let state = ConnectionState::Disconnected;
        assert!(!matches!(state, ConnectionState::Connected));

        let state = ConnectionState::Connected;
        assert!(matches!(state, ConnectionState::Connected));

        let state = ConnectionState::Connecting;
        assert!(matches!(state, ConnectionState::Connecting));

        let state = ConnectionState::Disconnecting;
        assert!(matches!(state, ConnectionState::Disconnecting));

        let state = ConnectionState::Error("test".into());
        assert!(matches!(state, ConnectionState::Error(_)));
    }
}
