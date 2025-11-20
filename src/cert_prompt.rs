use std::hash::Hasher;
use std::sync::{Arc, Mutex};

use flume::{Receiver, SendError, Sender};
use futures::stream::unfold;
use iced_futures::{
    subscription::{EventStream, Hasher as SubscriptionHasher, Recipe},
    BoxStream,
};
use once_cell::sync::OnceCell;

/// Decisions that can be made by the user when presented with a certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateDecision {
    TrustTemporarily,
    TrustPermanently,
    Reject,
}

/// Active profile metadata shared with the certificate verification callback.
#[derive(Debug, Clone)]
pub struct ActiveProfileInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
}

/// Channel used to signal the user's decision back to the callback.
pub type CertificateResponseTx = std::sync::mpsc::Sender<CertificateDecision>;

/// Information sent to the UI when a certificate prompt is emitted.
#[derive(Debug, Clone)]
pub struct CertificatePrompt {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub subject: String,
    pub issuer: String,
    pub fingerprint: String,
    pub pem: String,
    pub expired: bool,
    pub response_tx: CertificateResponseTx,
}

static PROMPT_SENDER: OnceCell<Sender<CertificatePrompt>> = OnceCell::new();
static ACTIVE_PROFILE: Mutex<Option<ActiveProfileInfo>> = Mutex::new(None);
static LAST_DECISION: Mutex<Option<CertificateDecision>> = Mutex::new(None);

/// Register a sender used by the SoftEther certificate callback.
pub fn register_sender(sender: Sender<CertificatePrompt>) -> Result<(), ()> {
    PROMPT_SENDER.set(sender).map_err(|_| ())
}

/// Dispatch a certificate prompt to the UI.
pub fn dispatch_prompt(prompt: CertificatePrompt) -> Result<(), SendError<CertificatePrompt>> {
    let sender = match PROMPT_SENDER.get() {
        Some(sender) => sender.clone(),
        None => return Err(SendError(prompt)),
    };
    sender.send(prompt)
}

/// Set or clear the active profile that is currently connecting.
pub fn set_active_profile(info: Option<ActiveProfileInfo>) {
    let mut guard = ACTIVE_PROFILE.lock().unwrap();
    *guard = info;
}

/// Clear the active profile once a connection attempt finishes.
pub fn clear_active_profile() {
    set_active_profile(None);
}

/// Get a copy of the active profile info, if any.
pub fn current_profile() -> Option<ActiveProfileInfo> {
    ACTIVE_PROFILE.lock().unwrap().clone()
}

/// Record the last certificate decision made by the user.
pub fn record_certificate_decision(decision: CertificateDecision) {
    let mut guard = LAST_DECISION.lock().unwrap();
    *guard = Some(decision);
}

/// Take ownership of the last recorded certificate decision, if any.
pub fn take_last_certificate_decision() -> Option<CertificateDecision> {
    let mut guard = LAST_DECISION.lock().unwrap();
    guard.take()
}

/// Clear any stored certificate decision (use before a new connect attempt).
pub fn clear_last_certificate_decision() {
    let mut guard = LAST_DECISION.lock().unwrap();
    *guard = None;
}

/// Build the subscription that streams certificate prompts into the UI.
pub fn subscription(receiver: Arc<Receiver<CertificatePrompt>>) -> CertificatePromptStreamRecipe {
    CertificatePromptStreamRecipe { receiver }
}

#[derive(Clone)]
pub struct CertificatePromptStreamRecipe {
    receiver: Arc<Receiver<CertificatePrompt>>,
}

impl Recipe for CertificatePromptStreamRecipe {
    type Output = CertificatePrompt;

    fn hash(&self, state: &mut SubscriptionHasher) {
        state.write_u64(0xCE11AC71);
    }

    fn stream(self: Box<Self>, _input: EventStream) -> BoxStream<Self::Output> {
        let receiver = self.receiver.clone();
        let stream = unfold(receiver, |receiver| async move {
            match receiver.recv_async().await {
                Ok(prompt) => Some((prompt, receiver)),
                Err(_) => None,
            }
        });
        Box::pin(stream)
    }
}
