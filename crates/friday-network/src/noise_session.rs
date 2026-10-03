/// FRIDAY Noise Protocol Session — `Noise_XX_25519_ChaChaPoly_BLAKE2s`
///
/// # Why Noise_XX?
/// - **IK** (pre-shared identity key): requires the responder's static public key to be
///   distributed in advance. Not suitable for first-time pairing.
/// - **XX** (mutual auth, no pre-shared static key): both parties exchange and authenticate
///   their long-term static keys during the handshake. Perfect for first pairing and
///   subsequent re-authentication when keys are already pinned in the trust store.
///
/// # Architecture
/// ```text
/// Pairing completed
///     ↓
/// Both devices generate Noise static key pair (if not already stored)
///     ↓
/// On every connection: Noise_XX handshake → both sides authenticated
///     ↓
/// NoiseSession wraps send/recv: all data is AEAD-encrypted with ChaCha20Poly1305
///     ↓
/// Data plane: still uses the same UDP socket — only encoding layer changes
/// ```
///
/// # Performance
/// - `snow` uses stack-allocated buffers for handshake messages (≤ 64KB packets).
/// - The transport state is `Box<TransportState>` (heap-allocated once per session).
/// - Each send/recv is a pure in-place AEAD operation with no dynamic allocation.
/// - Message overhead: 16 bytes (Poly1305 tag).
use snow::{Builder, TransportState};
use std::sync::Mutex;
use tracing::{debug, info};

/// Noise pattern for FRIDAY sessions.
/// XX = mutual authentication, Curve25519, ChaCha20Poly1305, BLAKE2s.
const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Maximum payload size for a single Noise transport message.
/// UDP practical limit ≈ 1400 bytes for LAN. Our mouse events are ≤ 64 bytes.
/// We allow up to 65000 bytes for bulk/clipboard payloads.
pub const NOISE_MAX_MSG: usize = 65000;

/// AEAD tag overhead added by Noise transport to every message (Poly1305).
pub const NOISE_TAG_BYTES: usize = 16;

/// Marker prefix so we can detect Noise-encrypted packets vs. legacy plain packets.
/// 4 bytes: 'N' 'O' 'I' 'S'
pub const NOISE_PACKET_MAGIC: &[u8; 4] = b"NOIS";

/// Statically-generated (or loaded) Noise static key pair for this installation.
/// The private key NEVER leaves this machine. The public key is shared during handshake.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoiseStaticKeyPair {
    /// Curve25519 private key (32 bytes, base64-encoded for JSON storage)
    pub private_key_b64: String,
    /// Curve25519 public key (32 bytes, base64-encoded for JSON storage)
    pub public_key_b64: String,
}

impl NoiseStaticKeyPair {
    /// Generate a new Noise static key pair using the `snow` builder.
    pub fn generate() -> Result<Self, NoiseError> {
        let builder = Builder::new(NOISE_PARAMS.parse().map_err(|e| {
            NoiseError::InitError(format!("Invalid Noise params '{}': {}", NOISE_PARAMS, e))
        })?);

        let keypair = builder
            .generate_keypair()
            .map_err(|e| NoiseError::InitError(format!("Keypair generation failed: {}", e)))?;

        Ok(Self {
            private_key_b64: base64_encode(&keypair.private),
            public_key_b64: base64_encode(&keypair.public),
        })
    }

    /// Decode the private key bytes.
    pub fn private_key_bytes(&self) -> Result<Vec<u8>, NoiseError> {
        base64_decode(&self.private_key_b64)
    }

    /// Decode the public key bytes.
    pub fn public_key_bytes(&self) -> Result<Vec<u8>, NoiseError> {
        base64_decode(&self.public_key_b64)
    }
}

/// Active Noise transport session (post-handshake, full duplex).
/// Wraps `snow::TransportState` behind a `Mutex` to allow shared use across tasks.
pub struct NoiseSession {
    /// Internal snow transport state (AEAD encoder/decoder).
    transport: Mutex<TransportState>,
    /// Remote peer's static public key (pinned after handshake).
    pub remote_static_pubkey: Vec<u8>,
    /// Session direction: true = this side initiated the handshake.
    pub is_initiator: bool,
}

impl NoiseSession {
    /// Encrypt `plaintext` into `out_buf`. Returns the number of ciphertext bytes written.
    ///
    /// Layout: `NOISE_PACKET_MAGIC (4)` + `ciphertext (plaintext.len + NOISE_TAG_BYTES)`
    ///
    /// # SAFETY
    /// `out_buf` must be at least `plaintext.len() + NOISE_TAG_BYTES + 4` bytes long.
    pub fn encrypt(&self, plaintext: &[u8], out_buf: &mut [u8]) -> Result<usize, NoiseError> {
        let required = 4 + plaintext.len() + NOISE_TAG_BYTES;
        if out_buf.len() < required {
            return Err(NoiseError::BufferTooSmall {
                required,
                actual: out_buf.len(),
            });
        }
        out_buf[..4].copy_from_slice(NOISE_PACKET_MAGIC);
        let mut guard = self.transport.lock().unwrap();
        let written = guard
            .write_message(plaintext, &mut out_buf[4..])
            .map_err(|e| NoiseError::EncryptError(format!("{}", e)))?;
        Ok(4 + written)
    }

    /// Decrypt `ciphertext` (must start with NOISE_PACKET_MAGIC) into `out_buf`.
    /// Returns the number of plaintext bytes written.
    pub fn decrypt(&self, ciphertext: &[u8], out_buf: &mut [u8]) -> Result<usize, NoiseError> {
        if ciphertext.len() < 4 || &ciphertext[..4] != NOISE_PACKET_MAGIC {
            return Err(NoiseError::NotANoisePacket);
        }
        let mut guard = self.transport.lock().unwrap();
        let written = guard
            .read_message(&ciphertext[4..], out_buf)
            .map_err(|e| NoiseError::DecryptError(format!("{}", e)))?;
        Ok(written)
    }

    /// Convenience: decrypt and return as owned Vec<u8> (used in tests / control path).
    pub fn decrypt_to_vec(&self, ciphertext: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut buf = vec![0u8; NOISE_MAX_MSG];
        let n = self.decrypt(ciphertext, &mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}

impl std::fmt::Debug for NoiseSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NoiseSession")
            .field("is_initiator", &self.is_initiator)
            .field("remote_pubkey_len", &self.remote_static_pubkey.len())
            .finish()
    }
}

/// Conducts the Noise_XX handshake as the **initiator**.
///
/// Handshake messages are exchanged over the caller-supplied send/recv closures so this
/// function is transport-agnostic (UDP, TCP, in-memory — used in tests).
///
/// After a successful handshake, the remote peer's static public key is extracted and
/// available via `NoiseSession::remote_static_pubkey`. The caller should persist this key
/// in the `TrustedDevice` record so future sessions can detect key changes (key pinning).
pub fn handshake_initiator(
    local_keypair: &NoiseStaticKeyPair,
    mut send: impl FnMut(&[u8]) -> Result<(), NoiseError>,
    mut recv: impl FnMut() -> Result<Vec<u8>, NoiseError>,
) -> Result<NoiseSession, NoiseError> {
    let params = NOISE_PARAMS
        .parse()
        .map_err(|e| NoiseError::InitError(format!("Parse params: {}", e)))?;

    let private_key = local_keypair.private_key_bytes()?;

    let mut initiator = Builder::new(params)
        .local_private_key(&private_key)
        .build_initiator()
        .map_err(|e| NoiseError::InitError(format!("Build initiator: {}", e)))?;

    // → msg1 (e)
    let mut msg1 = vec![0u8; 128];
    let n = initiator
        .write_message(&[], &mut msg1)
        .map_err(|e| NoiseError::HandshakeError(format!("write msg1: {}", e)))?;
    send(&msg1[..n])?;
    debug!("Noise initiator: sent msg1 ({} bytes)", n);

    // ← msg2 (e, ee, s, es)
    let msg2 = recv()?;
    let mut payload_buf = vec![0u8; 256];
    initiator
        .read_message(&msg2, &mut payload_buf)
        .map_err(|e| NoiseError::HandshakeError(format!("read msg2: {}", e)))?;
    debug!("Noise initiator: received msg2 ({} bytes)", msg2.len());

    // → msg3 (s, se)
    let mut msg3 = vec![0u8; 128];
    let n = initiator
        .write_message(&[], &mut msg3)
        .map_err(|e| NoiseError::HandshakeError(format!("write msg3: {}", e)))?;
    send(&msg3[..n])?;
    debug!("Noise initiator: sent msg3 ({} bytes)", n);

    // Transition to transport mode
    let remote_static = initiator
        .get_remote_static()
        .map(|k| k.to_vec())
        .unwrap_or_default();

    let transport = initiator
        .into_transport_mode()
        .map_err(|e| NoiseError::HandshakeError(format!("transport mode: {}", e)))?;

    info!(
        "Noise_XX handshake complete (initiator). Remote pubkey pinned ({} bytes).",
        remote_static.len()
    );

    Ok(NoiseSession {
        transport: Mutex::new(transport),
        remote_static_pubkey: remote_static,
        is_initiator: true,
    })
}

/// Conducts the Noise_XX handshake as the **responder**.
///
/// Mirror of `handshake_initiator`. Same transport-agnostic interface.
pub fn handshake_responder(
    local_keypair: &NoiseStaticKeyPair,
    mut send: impl FnMut(&[u8]) -> Result<(), NoiseError>,
    mut recv: impl FnMut() -> Result<Vec<u8>, NoiseError>,
) -> Result<NoiseSession, NoiseError> {
    let params = NOISE_PARAMS
        .parse()
        .map_err(|e| NoiseError::InitError(format!("Parse params: {}", e)))?;

    let private_key = local_keypair.private_key_bytes()?;

    let mut responder = Builder::new(params)
        .local_private_key(&private_key)
        .build_responder()
        .map_err(|e| NoiseError::InitError(format!("Build responder: {}", e)))?;

    // ← msg1 (e)
    let msg1 = recv()?;
    let mut payload_buf = vec![0u8; 256];
    responder
        .read_message(&msg1, &mut payload_buf)
        .map_err(|e| NoiseError::HandshakeError(format!("read msg1: {}", e)))?;
    debug!("Noise responder: received msg1 ({} bytes)", msg1.len());

    // → msg2 (e, ee, s, es)
    let mut msg2 = vec![0u8; 256];
    let n = responder
        .write_message(&[], &mut msg2)
        .map_err(|e| NoiseError::HandshakeError(format!("write msg2: {}", e)))?;
    send(&msg2[..n])?;
    debug!("Noise responder: sent msg2 ({} bytes)", n);

    // ← msg3 (s, se)
    let msg3 = recv()?;
    let mut payload_buf2 = vec![0u8; 256];
    responder
        .read_message(&msg3, &mut payload_buf2)
        .map_err(|e| NoiseError::HandshakeError(format!("read msg3: {}", e)))?;
    debug!("Noise responder: received msg3 ({} bytes)", msg3.len());

    // Extract remote static key before consuming HandshakeState
    let remote_static = responder
        .get_remote_static()
        .map(|k| k.to_vec())
        .unwrap_or_default();

    let transport = responder
        .into_transport_mode()
        .map_err(|e| NoiseError::HandshakeError(format!("transport mode: {}", e)))?;

    info!(
        "Noise_XX handshake complete (responder). Remote pubkey pinned ({} bytes).",
        remote_static.len()
    );

    Ok(NoiseSession {
        transport: Mutex::new(transport),
        remote_static_pubkey: remote_static,
        is_initiator: false,
    })
}

/// Check if a raw packet byte slice is a Noise-encrypted packet.
#[inline]
pub fn is_noise_packet(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == NOISE_PACKET_MAGIC
}

/// Errors in Noise session lifecycle.
#[derive(Debug, thiserror::Error)]
pub enum NoiseError {
    #[error("Noise init error: {0}")]
    InitError(String),

    #[error("Noise handshake error: {0}")]
    HandshakeError(String),

    #[error("Noise encrypt error: {0}")]
    EncryptError(String),

    #[error("Noise decrypt error: {0}")]
    DecryptError(String),

    #[error("Packet is not a Noise-encrypted packet (missing NOIS magic)")]
    NotANoisePacket,

    #[error("Output buffer too small: need {required}, have {actual}")]
    BufferTooSmall { required: usize, actual: usize },

    #[error("Key encoding error: {0}")]
    KeyError(String),

    #[error("Transport error: {0}")]
    TransportError(String),
}

// ── Internal base64 helpers (avoids adding another crate) ─────────────────────

fn base64_encode(bytes: &[u8]) -> String {
    // Simple base64 without padding using URL-safe alphabet is unnecessary;
    // use standard alphabet for interoperability.
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };

        out.push(TABLE[(b0 >> 2) & 0x3f] as char);
        out.push(TABLE[((b0 << 4) | (b1 >> 4)) & 0x3f] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((b1 << 2) | (b2 >> 6)) & 0x3f] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[b2 & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, NoiseError> {
    fn val(c: u8) -> Result<u8, NoiseError> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            b'=' => Ok(0),
            _ => Err(NoiseError::KeyError(format!(
                "Invalid base64 char: {}",
                c as char
            ))),
        }
    }

    let s = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut i = 0;
    while i + 3 < s.len() {
        let a = val(s[i])?;
        let b = val(s[i + 1])?;
        let c = val(s[i + 2])?;
        let d = val(s[i + 3])?;

        out.push((a << 2) | (b >> 4));
        if s[i + 2] != b'=' {
            out.push((b << 4) | (c >> 2));
        }
        if s[i + 3] != b'=' {
            out.push((c << 6) | d);
        }
        i += 4;
    }
    Ok(out)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates two pairs of (send, recv) closures backed by blocking mpsc channels.
    /// `(send_a, recv_a)` — side A's I/O (A sends → B receives via `recv_b`)
    /// `(send_b, recv_b)` — side B's I/O (B sends → A receives via `recv_a`)
    ///
    /// `recv_*` blocks for up to 5 seconds — long enough for any in-process handshake.
    fn channel_pair() -> (
        Box<dyn FnMut(&[u8]) -> Result<(), NoiseError> + Send>,
        Box<dyn FnMut() -> Result<Vec<u8>, NoiseError> + Send>,
        Box<dyn FnMut(&[u8]) -> Result<(), NoiseError> + Send>,
        Box<dyn FnMut() -> Result<Vec<u8>, NoiseError> + Send>,
    ) {
        use std::sync::mpsc;
        use std::time::Duration;

        let (a_to_b_tx, a_to_b_rx) = mpsc::channel::<Vec<u8>>();
        let (b_to_a_tx, b_to_a_rx) = mpsc::channel::<Vec<u8>>();

        let send_a = move |data: &[u8]| -> Result<(), NoiseError> {
            a_to_b_tx
                .send(data.to_vec())
                .map_err(|e| NoiseError::TransportError(e.to_string()))
        };
        let recv_a = move || -> Result<Vec<u8>, NoiseError> {
            b_to_a_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| NoiseError::TransportError(format!("recv_a: {}", e)))
        };
        let send_b = move |data: &[u8]| -> Result<(), NoiseError> {
            b_to_a_tx
                .send(data.to_vec())
                .map_err(|e| NoiseError::TransportError(e.to_string()))
        };
        let recv_b = move || -> Result<Vec<u8>, NoiseError> {
            a_to_b_rx
                .recv_timeout(Duration::from_secs(5))
                .map_err(|e| NoiseError::TransportError(format!("recv_b: {}", e)))
        };

        (
            Box::new(send_a),
            Box::new(recv_a),
            Box::new(send_b),
            Box::new(recv_b),
        )
    }

    #[test]
    fn test_noise_keypair_generation() {
        let kp = NoiseStaticKeyPair::generate().unwrap();
        assert!(!kp.private_key_b64.is_empty());
        assert!(!kp.public_key_b64.is_empty());
        let priv_bytes = kp.private_key_bytes().unwrap();
        let pub_bytes = kp.public_key_bytes().unwrap();
        // Curve25519 keys are always 32 bytes
        assert_eq!(priv_bytes.len(), 32);
        assert_eq!(pub_bytes.len(), 32);
        // Private key ≠ public key
        assert_ne!(priv_bytes, pub_bytes);
    }

    #[test]
    fn test_noise_xx_handshake_and_transport() {
        let kp_a = NoiseStaticKeyPair::generate().unwrap();
        let kp_b = NoiseStaticKeyPair::generate().unwrap();

        // Both sides must run concurrently — the protocol blocks on each recv.
        let (mut send_a, mut recv_a, mut send_b, mut recv_b) = channel_pair();

        let kp_a_clone = kp_a.clone();
        let kp_b_clone = kp_b.clone();

        let initiator_handle =
            std::thread::spawn(move || handshake_initiator(&kp_a_clone, &mut send_a, &mut recv_a));
        // Responder runs on the test thread while initiator runs in background.
        let responder_session = handshake_responder(&kp_b, &mut send_b, &mut recv_b).unwrap();
        let initiator_session = initiator_handle.join().unwrap().unwrap();

        // Cross-verify pinned remote public keys (key pinning)
        let kp_a_pub = kp_a.public_key_bytes().unwrap();
        let kp_b_pub = kp_b_clone.public_key_bytes().unwrap();
        assert_eq!(
            responder_session.remote_static_pubkey, kp_a_pub,
            "responder should pin initiator's pubkey"
        );
        assert_eq!(
            initiator_session.remote_static_pubkey, kp_b_pub,
            "initiator should pin responder's pubkey"
        );

        // Initiator → responder
        let plaintext = b"FRIDAY mouse event: (dx=10, dy=-5)";
        let mut cipher = vec![0u8; plaintext.len() + NOISE_TAG_BYTES + 4];
        let n = initiator_session.encrypt(plaintext, &mut cipher).unwrap();
        let decrypted = responder_session.decrypt_to_vec(&cipher[..n]).unwrap();
        assert_eq!(decrypted.as_slice(), plaintext);

        // Responder → initiator
        let plaintext2 = b"ACK";
        let mut cipher2 = vec![0u8; plaintext2.len() + NOISE_TAG_BYTES + 4];
        let n2 = responder_session.encrypt(plaintext2, &mut cipher2).unwrap();
        let decrypted2 = initiator_session.decrypt_to_vec(&cipher2[..n2]).unwrap();
        assert_eq!(decrypted2.as_slice(), plaintext2);
    }

    #[test]
    fn test_noise_packet_magic_detection() {
        assert!(is_noise_packet(b"NOISsomedata"));
        assert!(!is_noise_packet(b"FRDYsomedata")); // legacy plain packet
        assert!(!is_noise_packet(b"NO")); // too short
        assert!(!is_noise_packet(b"")); // empty
    }

    #[test]
    fn test_noise_decrypt_reject_non_noise_packet() {
        let kp_a = NoiseStaticKeyPair::generate().unwrap();
        let kp_b = NoiseStaticKeyPair::generate().unwrap();

        let (mut send_a, mut recv_a, mut send_b, mut recv_b) = channel_pair();
        let kp_a_clone = kp_a.clone();

        let initiator_handle =
            std::thread::spawn(move || handshake_initiator(&kp_a_clone, &mut send_a, &mut recv_a));
        let responder_session = handshake_responder(&kp_b, &mut send_b, &mut recv_b).unwrap();
        let _ = initiator_handle.join().unwrap().unwrap();

        // A legacy FRDY packet (no NOIS magic) must be rejected outright
        let fake_packet = b"FRDYsomemaliciousdata";
        let mut out = vec![0u8; 1024];
        let result = responder_session.decrypt(fake_packet, &mut out);
        assert!(
            matches!(result, Err(NoiseError::NotANoisePacket)),
            "expected NotANoisePacket error"
        );
    }

    #[test]
    fn test_base64_encode_decode_roundtrip() {
        let original = b"FRIDAY Noise Key Material 32-byte";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(&decoded, original);
    }
}
