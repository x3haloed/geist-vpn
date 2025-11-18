//! VPN Manager for thread-safe SoftEther operations
//!
//! This module provides a safe interface to SoftEtherVPN operations
//! by running all FFI calls in a dedicated task/thread.

use crate::ConnectionStatus;
use geist_vpn::client::SoftEtherClient;
use geist_vpn::profile::VpnProfile;
use std::sync::mpsc;

/// Commands that can be sent to the VPN manager
#[derive(Debug)]
pub enum VpnCommand {
    Connect {
        profile: VpnProfile,
        response_tx: mpsc::Sender<Result<(), String>>,
    },
    Disconnect {
        response_tx: mpsc::Sender<Result<(), String>>,
    },
    GetStatus {
        response_tx: mpsc::Sender<ConnectionStatus>,
    },
    Shutdown,
}

/// VPN Manager that handles SoftEther operations safely
pub struct VpnManager {
    command_tx: mpsc::Sender<VpnCommand>,
    _handle: std::thread::JoinHandle<()>,
}

impl VpnManager {
    /// Create a new VPN manager
    pub fn new() -> Result<Self, String> {
        tracing::info!("VPN Manager: Creating new VPN manager");
        let (command_tx, command_rx) = mpsc::channel();

        tracing::info!("VPN Manager: Spawning background thread");
        let handle = std::thread::spawn(move || {
            Self::run_manager(command_rx);
        });

        tracing::info!("VPN Manager: Background thread spawned successfully");
        Ok(Self {
            command_tx,
            _handle: handle,
        })
    }

    /// Connect to a VPN using the specified profile
    pub fn connect(&self, profile: VpnProfile) -> Result<(), String> {
        let (response_tx, response_rx) = mpsc::channel();

        self.command_tx
            .send(VpnCommand::Connect { profile, response_tx })
            .map_err(|e| format!("Failed to send connect command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive connect response: {}", e))?
    }

    /// Disconnect from the current VPN connection
    pub fn disconnect(&self) -> Result<(), String> {
        let (response_tx, response_rx) = mpsc::channel();

        self.command_tx
            .send(VpnCommand::Disconnect { response_tx })
            .map_err(|e| format!("Failed to send disconnect command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive disconnect response: {}", e))?
    }

    /// Get the current connection status
    pub fn get_status(&self) -> Result<ConnectionStatus, String> {
        let (response_tx, response_rx) = mpsc::channel();

        self.command_tx
            .send(VpnCommand::GetStatus { response_tx })
            .map_err(|e| format!("Failed to send status command: {}", e))?;

        response_rx
            .recv()
            .map_err(|e| format!("Failed to receive status response: {}", e))
    }

    /// Run the VPN manager loop (called in dedicated thread)
    fn run_manager(command_rx: mpsc::Receiver<VpnCommand>) {
        tracing::info!("VPN Manager: Background thread started, creating Tokio runtime");

        // Create a Tokio runtime for this thread
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!("VPN Manager: Failed to create Tokio runtime: {}", e);
                return;
            }
        };

        tracing::info!("VPN Manager: Tokio runtime created, creating SoftEther client");

        // Create the SoftEther client (this will initialize SoftEther if needed)
        let mut client = match SoftEtherClient::new() {
            Ok(client) => {
                tracing::info!("VPN Manager: SoftEther client created successfully");
                client
            }
            Err(e) => {
                tracing::error!("VPN Manager: Failed to create SoftEther client: {}", e);
                return;
            }
        };

        tracing::info!("VPN Manager: VPN manager initialized and ready, entering command loop");

        // Main command loop - use catch_unwind to prevent thread from panicking
        while let Ok(command) = command_rx.recv() {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                match command {
                    VpnCommand::Connect { profile, response_tx } => {
                        tracing::info!("VPN manager: Connecting to {}", profile.name);

                        // Run the connect operation using the Tokio runtime
                        let result = rt.block_on(async {
                            client.connect(&profile).await
                        });

                        let _ = response_tx.send(result.map_err(|e| format!("Connection failed: {}", e)));
                    }

                    VpnCommand::Disconnect { response_tx } => {
                        tracing::info!("VPN manager: Disconnecting");

                        // Run the disconnect operation using the Tokio runtime
                        let result = rt.block_on(async {
                            client.disconnect().await
                        });

                        let _ = response_tx.send(result.map_err(|e| format!("Disconnection failed: {}", e)));
                    }

                    VpnCommand::GetStatus { response_tx } => {
                        let client_status = client.get_status();

                        // Convert client status to UI status
                        let ui_status = match client_status {
                            geist_vpn::client::ConnectionStatus::Disconnected => {
                                ConnectionStatus {
                                    connected: false,
                                    profile_name: None,
                                    status_message: "Disconnected".to_string(),
                                }
                            }
                            geist_vpn::client::ConnectionStatus::Connecting => {
                                ConnectionStatus {
                                    connected: false,
                                    profile_name: client.active_profile().map(|p| p.name.clone()),
                                    status_message: "Connecting...".to_string(),
                                }
                            }
                            geist_vpn::client::ConnectionStatus::Connected => {
                                ConnectionStatus {
                                    connected: true,
                                    profile_name: client.active_profile().map(|p| p.name.clone()),
                                    status_message: "Connected".to_string(),
                                }
                            }
                            geist_vpn::client::ConnectionStatus::Disconnecting => {
                                ConnectionStatus {
                                    connected: true,
                                    profile_name: client.active_profile().map(|p| p.name.clone()),
                                    status_message: "Disconnecting...".to_string(),
                                }
                            }
                            geist_vpn::client::ConnectionStatus::Error(msg) => {
                                ConnectionStatus {
                                    connected: false,
                                    profile_name: None,
                                    status_message: format!("Error: {}", msg),
                                }
                            }
                        };

                        let _ = response_tx.send(ui_status);
                    }

                    VpnCommand::Shutdown => {
                        tracing::info!("VPN manager shutting down");

                        // Attempt to disconnect if connected
                        let _ = rt.block_on(async {
                            client.disconnect().await
                        });

                        // Cleanup SoftEther
                        let _ = geist_vpn::cleanup();

                        return;
                    }
                }
            }));

            // Handle any panics in the command processing
            if let Err(panic_info) = result {
                tracing::error!("VPN Manager: Command processing panicked: {:?}", panic_info);
                // Continue the loop - don't let one bad command kill the manager
            }
        }

        tracing::info!("VPN manager thread exiting");
    }
}

impl Drop for VpnManager {
    fn drop(&mut self) {
        // Send shutdown command
        let _ = self.command_tx.send(VpnCommand::Shutdown);
    }
}
