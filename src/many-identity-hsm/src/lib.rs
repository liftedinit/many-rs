use coset::{CoseKey, CoseSign1, CoseSign1Builder};
use cryptoki::context::{CInitializeArgs, Pkcs11};
use cryptoki::mechanism::{Mechanism, MechanismType};
use cryptoki::object::{Attribute, AttributeType, ObjectHandle};
use cryptoki::session::{Session, SessionFlags, UserType};
use cryptoki::slot::Slot;
use many_error::ManyError;
use many_identity::cose::add_keyset_header;
use many_identity::{cose, Address};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use tracing::{error, trace};

/// HSM Singleton
/// PKCS#11 v2.40 specifies that
///
/// "An application should never make multiple simultaneous function call to
/// Cryptoki which use a common session. If multiple threads of an application
/// attempt to use a common session concurrently in this fashion, Cryptoki does
/// not define what happens. This means that if a multiple threads of an
/// application all need to use Cryptoki to access a particular token, it might
/// be appropriate for each thread to have its own session with the token,
/// unless the application can ensure by some other means (e.g., by some locking
/// mechanism) that no sessions are ever used by multiple threads
/// simultaneously."
///
/// If one ever modify this behavior, make sure that the application/tests don't
/// hit the Cryptoki simultaneously
static HSM_INSTANCE: Lazy<Mutex<Hsm>> = Lazy::new(|| Mutex::new(Hsm::default()));

/// Same as cryptoki::session::UserType
pub type HsmUserType = UserType;

/// Same as cryptoki::mechanism::Mechanism
pub type HsmMechanism = Mechanism;

/// Same as cryptoki::mechanism::MechanismType
pub type HsmMechanismType = MechanismType;

/// HSM session type.
pub enum HsmSessionType {
    /// Read-only
    RO,
    /// Read-write
    RW,
}

/// Holds the PKCS#11 context, the PKCS#11 session and the HSM Key ID to use to
/// perform the cryptographic operations
#[derive(Debug, Default)]
pub struct Hsm {
    pkcs11: Option<Pkcs11>,
    session: Option<Session>,
    keyid: Option<Vec<u8>>,
}

impl Hsm {
    /// Return the HSM global instance
    pub fn get_instance() -> Result<MutexGuard<'static, Hsm>, ManyError> {
        HSM_INSTANCE.lock().map_err(ManyError::hsm_mutex_poisoned)
    }

    /// Perform message signature on the HSM using the given mechanism
    ///
    /// Note: The NIST P-256 curve requires the user to hash the message with
    /// SHA256, and to sign the result.
    pub fn sign(&self, msg: &[u8], mechanism: &HsmMechanism) -> Result<Vec<u8>, ManyError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| ManyError::hsm_session_error("No PKCS#11 open session found"))?;

        let signer = self.signer()?;
        trace!("Signing message using HSM");
        let signature = session
            .sign(mechanism, signer, msg)
            .map_err(ManyError::hsm_sign_error)?;
        Ok(signature)
    }

    /// Return the object handle of the HSM singing key (private key)
    fn signer(&self) -> Result<ObjectHandle, ManyError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| ManyError::hsm_session_error("No PKCS#11 open session found"))?;
        let keyid = self
            .keyid
            .as_ref()
            .ok_or_else(|| ManyError::hsm_keyid_error("No PKCS#11 key ID found"))?;

        trace!("Looking for private key");
        let template = &[Attribute::Id(keyid.clone()), Attribute::Sign(true)];
        let mut signers = session
            .find_objects(template)
            .map_err(|e| ManyError::hsm_sign_error(format!("{e}")))?;

        trace!("Making sure we found one and only one private key");
        let signer = match signers.len() {
            0 => {
                panic!("Unable to find private key")
            }
            1 => signers
                .pop()
                .ok_or_else(|| ManyError::hsm_sign_error("Unable to fetch private key"))?,
            _ => {
                panic!("Multiple private key found")
            }
        };
        Ok(signer)
    }

    /// Perform message signature verification on the HSM using the given mechanism
    ///
    /// Note: The NIST P-256 curve requires the user to hash the message with
    /// SHA256, and to verify the result.
    pub fn verify(
        &self,
        msg: &[u8],
        signature: &[u8],
        mechanism: &HsmMechanism,
    ) -> Result<(), ManyError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| ManyError::hsm_session_error("No PKCS#11 open session found"))?;

        let verifier = self.verifier()?;
        session
            .verify(mechanism, verifier, msg, signature)
            .map_err(|e| ManyError::hsm_verify_error(format!("{e}")))?;
        Ok(())
    }

    /// Return the object handle of the HSM verification key (public key)
    fn verifier(&self) -> Result<ObjectHandle, ManyError> {
        let session = self
            .session
            .as_ref()
            .ok_or_else(|| ManyError::hsm_session_error("No PKCS#11 open session found"))?;
        let keyid = self
            .keyid
            .as_ref()
            .ok_or_else(|| ManyError::hsm_keyid_error("No PKCS#11 key ID found".to_string()))?;

        trace!("Looking for public key");
        let template = &[Attribute::Id(keyid.clone()), Attribute::Verify(true)];
        let mut verifiers = session
            .find_objects(template)
            .map_err(|e| ManyError::hsm_verify_error(format!("{e}")))?;

        trace!("Making sure we found one and only one public key");
        let verifier = match verifiers.len() {
            0 => {
                panic!("Unable to find public key")
            }
            1 => verifiers.pop().ok_or_else(|| {
                ManyError::hsm_verify_error("Unable to fetch public key".to_string())
            })?,
            _ => {
                panic!("Multiple public key found")
            }
        };
        Ok(verifier)
    }

    /// Retrieve the EC_POINT and EC_PARAMS key parameters
    ///
    /// EC_POINT is returned in raw, uncompressed form, i.e., NOT ASN.1 DER
    ///
    /// Note: Only works with EC keys
    pub fn ec_info(&self, mechanism: HsmMechanismType) -> Result<(Vec<u8>, Vec<u8>), ManyError> {
        let pkcs11 = self
            .pkcs11
            .as_ref()
            .ok_or_else(|| ManyError::hsm_init_error("No PKCS#11 context found.".to_string()))?;
        let session = self.session.as_ref().ok_or_else(|| {
            ManyError::hsm_session_error("No PKCS#11 open session found".to_string())
        })?;

        trace!("Making sure we can fetch uncompressed EC_POINT");
        let slot = session
            .get_session_info()
            .map_err(|e| ManyError::hsm_ec_point_error(e.to_string()))?
            .slot_id();
        let uncompress = pkcs11
            .get_mechanism_info(slot, mechanism)
            .map_err(|e| ManyError::hsm_ec_point_error(e.to_string()))?
            .flags()
            .ec_uncompress();
        if !uncompress {
            panic!("Could not fetch uncompressed EC_POINT");
        }

        let verifier = self.verifier()?;
        let results = session
            .get_attributes(verifier, &[AttributeType::EcPoint])
            .map_err(|e| ManyError::hsm_ec_point_error(format!("{e}")))?;
        let ec_points = if let Some(Attribute::EcPoint(points)) = results.get(0) {
            points
        } else {
            panic!("Public EC point attribute not available")
        };

        trace!("Fetching EC public key params");
        let results = session
            .get_attributes(verifier, &[AttributeType::EcParams])
            .map_err(|e| ManyError::hsm_ec_params_error(format!("{e}")))?;
        let ec_params = if let Some(Attribute::EcParams(params)) = results.get(0) {
            params
        } else {
            panic!("Public EC params attribute not available")
        };

        trace!("Decoding EC_POINT using ASN.1 DER");
        let raw_points: &[u8] = asn1::parse_single(ec_points)
            .map_err(|e| ManyError::hsm_ec_point_error(format!("{e:?}")))?;
        trace!("Raw, uncompressed EC_POINT: {}", hex::encode(raw_points));
        Ok((raw_points.to_vec(), ec_params.clone()))
    }

    /// Initialize the PKCS#11 context and set the HSM keyid. You should run
    /// this only once at the beginning of your application
    pub fn init(&mut self, module: PathBuf, keyid: Vec<u8>) -> Result<(), ManyError> {
        match &self.pkcs11 {
            None => {
                trace!("Loading and initializing PKCS#11 module");
                let pkcs11 = Pkcs11::new(module).expect("Unable to load PKCS#11 module");
                pkcs11
                    .initialize(CInitializeArgs::OsThreads)
                    .map_err(|e| ManyError::hsm_init_error(e.to_string()))?;
                self.pkcs11.replace(pkcs11);
                trace!("PKCS#11 context initialized");
            }
            Some(_) => {
                error!("PKCS#11 context already initialized!");
            }
        }

        match &self.keyid {
            None => {
                self.keyid.replace(keyid);
                trace!("keyid initialized");
            }
            Some(_) => {
                error!("Key ID already initialized!");
            }
        }
        Ok(())
    }

    /// Open a session on the HSM
    ///
    /// Public RO session and private RO/RW sessions are supported
    /// Read-only (RO) and read-write (RW) serial sessions are supported
    pub fn open_session(
        &mut self,
        slot: u64,
        session_type: HsmSessionType,
        user_type: Option<HsmUserType>,
        pin: Option<String>,
    ) -> Result<(), ManyError> {
        let pkcs11 = self
            .pkcs11
            .as_ref()
            .ok_or_else(|| ManyError::hsm_init_error("No PKCS#11 context found.".to_string()))?;
        let slot = Slot::try_from(slot).map_err(|e| ManyError::hsm_session_error(e.to_string()))?;
        match &self.session {
            None => {
                let session_flags = match session_type {
                    // Read-only PKCS#11 session
                    HsmSessionType::RO => {
                        trace!("Creating RO session flags");
                        let mut flags = SessionFlags::new();
                        flags.set_serial_session(true);
                        flags
                    }
                    // Read-write PKCS#11 session
                    HsmSessionType::RW => {
                        trace!("Creating RW session flags");
                        let mut flags = SessionFlags::new();
                        flags.set_serial_session(true).set_rw_session(true);
                        flags
                    }
                };
                trace!("Opening HSM session");
                let session = pkcs11
                    .open_session_no_callback(slot, session_flags)
                    .map_err(|e| ManyError::hsm_session_error(format!("{e}")))?;

                // A user type means that the user needs to login
                match user_type {
                    None => {}
                    Some(u) => {
                        trace!("Login user to HSM as {:?}", u);
                        session
                            .login(u, pin.as_deref())
                            .map_err(|e| ManyError::hsm_login_error(format!("{e}")))?;
                    }
                }
                trace!("Session to HSM opened successfully");
                self.session.replace(session);
            }
            Some(_) => {
                error!("A session is already opened!");
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct HsmIdentity {
    address: Address,
    key: CoseKey,
}

impl HsmIdentity {
    pub fn new(mechanism: HsmMechanismType) -> Result<Self, ManyError> {
        let hsm = Hsm::get_instance()?;
        let (raw_points, _) = hsm.ec_info(mechanism)?;
        trace!("Creating NIST P-256 SEC1 encoded point");
        let points = p256::EncodedPoint::from_bytes(raw_points).map_err(ManyError::unknown)?;

        let key = many_identity_dsa::ecdsa::ecdsa_cose_key(
            (points.x().unwrap().to_vec(), points.y().unwrap().to_vec()),
            None,
        );
        let public_key = many_identity_dsa::ecdsa::public_key(&key)?
            .ok_or_else(|| ManyError::unknown("Could not load key."))?;
        let address = unsafe { cose::address_unchecked(&public_key) }?;
        Ok(Self { address, key })
    }
}

impl many_identity::Identity for HsmIdentity {
    fn address(&self) -> Address {
        self.address
    }

    fn public_key(&self) -> Option<CoseKey> {
        Some(self.key.clone())
    }

    fn sign_1(&self, envelope: CoseSign1) -> Result<CoseSign1, ManyError> {
        let hsm = Hsm::get_instance()?;
        let mut envelope = add_keyset_header(envelope, self)?;

        // Add the algorithm and key id.
        envelope.protected.header.alg =
            Some(coset::Algorithm::Assigned(coset::iana::Algorithm::ES256));
        envelope.protected.header.key_id = self.address.to_vec();

        let builder = CoseSign1Builder::new()
            .protected(envelope.protected.header)
            .unprotected(envelope.unprotected);

        let builder = if let Some(payload) = envelope.payload {
            builder.payload(payload)
        } else {
            builder
        };

        Ok(builder
            .try_create_signature(&[], |bytes| {
                use sha2::Digest;

                trace!("Digesting message using SHA256 (CPU)");
                let digest = sha2::Sha256::digest(bytes);

                trace!("Singning message using HSM");
                let msg_signature = hsm.sign(digest.as_slice(), &HsmMechanism::Ecdsa)?;
                trace!("Message signature is {}", hex::encode(&msg_signature));

                Ok(msg_signature)
            })?
            .build())
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use cryptoki::{
        object::{KeyType, ObjectHandle},
        session::SessionState,
    };
    use p256::ecdsa::signature::Verifier;
    use sha2::Digest;

    use super::*;

    type HSMAttribute = Attribute;
    type HSMObjectHandle = ObjectHandle;

    const KEYPAIR_TEST_ID: &[u8] = &[15, 15];
    const SO_PIN: &str = "0000";
    const USER_PIN: &str = "0000";
    const MSG: &str = "FOOBAR";
    // 1.2.840.10045.3.1.7
    const SECP256R1_OID: &[u8] = &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];
    static ECDSA_PUB_KEY_TEMPLATE: Lazy<Vec<Attribute>> = Lazy::new(|| {
        vec![
            Attribute::Token(true),
            Attribute::Private(false),
            Attribute::KeyType(KeyType::EC),
            Attribute::Verify(true),
            Attribute::EcParams(SECP256R1_OID.to_vec()),
            Attribute::Id(KEYPAIR_TEST_ID.to_vec()),
        ]
    });
    static ECDSA_PRIV_KEY_TEMPLATE: Lazy<Vec<Attribute>> = Lazy::new(|| {
        vec![
            Attribute::Token(true),
            Attribute::Private(true),
            Attribute::Sensitive(true),
            Attribute::Extractable(false),
            Attribute::Sign(true),
            Attribute::Id(KEYPAIR_TEST_ID.to_vec()),
        ]
    });

    /// HSM methods only used for testing purposes
    impl Hsm {
        /// Initialize user PIN using an SO FW session
        fn init_user_pin(&self, pin: String) -> Result<(), ManyError> {
            match &self.session {
                None => {
                    panic!("You need to open a SO session in order to initialize the user PIN")
                }
                Some(s) => {
                    let info = s
                        .get_session_info()
                        .map_err(|e| ManyError::hsm_session_error(format!("{e}")))?;
                    match info.session_state() {
                        SessionState::RW_SO_FUNCTIONS => {
                            self.session
                                .as_ref()
                                .ok_or_else(|| {
                                    ManyError::hsm_session_error(
                                        "Unable to access session".to_string(),
                                    )
                                })?
                                .init_pin(&pin)
                                .map_err(|e| ManyError::hsm_session_error(format!("{e}")))?;
                        }
                        _ => {
                            panic!(
                                "You need to open a SO session in order to initialize the user PIN"
                            )
                        }
                    }
                }
            }
            Ok(())
        }

        /// Initialize a new token on the HSM
        fn init_token(&self, slot: Slot, pin: String, label: String) -> Result<(), ManyError> {
            match &self.pkcs11 {
                None => {
                    panic!("PKCS#11 context not initialized")
                }
                Some(_) => {
                    self.pkcs11
                        .as_ref()
                        .ok_or_else(|| {
                            ManyError::hsm_init_error(
                                "Unable to access PKCS#11 context".to_string(),
                            )
                        })?
                        .init_token(slot, &pin, &label)
                        .map_err(|e| ManyError::hsm_init_error(format!("{e}")))?;
                }
            }

            Ok(())
        }

        /// Generate a new keypair on the HSM
        fn generate_key_pair(
            &self,
            mechanism: &HsmMechanism,
            pub_template: &[HSMAttribute],
            priv_template: &[HSMAttribute],
        ) -> Result<(HSMObjectHandle, HSMObjectHandle), ManyError> {
            let session = self.session.as_ref().ok_or_else(|| {
                ManyError::hsm_session_error("No PKCS#11 open session found".to_string())
            })?;

            session
                .generate_key_pair(mechanism, pub_template, priv_template)
                .map_err(|e| ManyError::hsm_keygen_error(format!("{e}")))
        }

        /// Destroy test keys after test run
        fn destroy(&self, obj: ObjectHandle) -> Result<(), ManyError> {
            let session = self.session.as_ref().ok_or_else(|| {
                ManyError::hsm_session_error("No PKCS#11 open session found".to_string())
            })?;

            session
                .destroy_object(obj)
                .map_err(|e| ManyError::hsm_session_error(e.to_string()))?;
            Ok(())
        }

        /// Close the HSM session
        fn close_session(&mut self) {
            self.session = None;
        }
    }

    /// Setup the testing HSM environment
    ///
    /// The PKCS#11 module path can be specified using the
    /// `PKCS11_SOFTHSM2_MODULE` environment variable. By default, it will try
    /// to load `/usr/lib/softhsm/libsofthsm2.so
    ///
    /// # Example
    ///
    /// $ PKCS11_SOFTHSM2_MODULE=/usr/local/lib/softhsm/libsofthsm2.so cargo test
    ///
    /// This function will
    /// - Creates and initialize a PKCS#11 context
    /// - Initialize a new token and set SO PIN
    /// - Open a SO RW session
    ///     - Set the user PIN
    /// - Open a user RW session
    ///     - Generate a new ECDSA (secp256r1) keypair
    /// - Return the slot number
    fn init() -> Result<u64, ManyError> {
        let mut hsm = Hsm::get_instance()?;
        let module = env::var("PKCS11_SOFTHSM2_MODULE")
            .unwrap_or_else(|_| "/usr/lib/softhsm/libsofthsm2.so".to_string());
        hsm.init(PathBuf::from(module), KEYPAIR_TEST_ID.to_vec())
            .expect("Unable to init PKCS#11");
        let pkcs11 = hsm.pkcs11.as_ref().ok_or_else(|| {
            ManyError::hsm_init_error("Unable to access PKCS#11 context".to_string())
        })?;

        let mut slots = pkcs11
            .get_slots_with_token()
            .map_err(|e| ManyError::hsm_init_error(e.to_string()))?;
        let slot = slots.pop().ok_or_else(|| {
            ManyError::hsm_session_error("Unable to fetch slots with token".to_string())
        })?;
        hsm.init_token(slot, SO_PIN.to_string(), "Test Token".to_string())?;
        let slot = slot.id();

        hsm.open_session(
            slot,
            HsmSessionType::RW,
            Some(HsmUserType::So),
            Some(SO_PIN.to_string()),
        )?;
        hsm.init_user_pin(USER_PIN.to_string())?;
        hsm.close_session();
        Ok(slot)
    }

    /// Test that message signing and signature verification works on the HSM
    ///
    /// This test will initialize a new token and generate a new ECDSA P256 keypair.
    /// The keypair will be destroyed at the end of the test, but the token will remain initialized.
    #[test]
    fn hsm_ecdsa_sign_verify() -> Result<(), ManyError> {
        let slot = init()?;

        let mut hsm = Hsm::get_instance()?;
        hsm.open_session(
            slot,
            HsmSessionType::RW, // We need to open a RW session since we're destroying the keys at the end of the test
            Some(HsmUserType::User),
            Some(USER_PIN.to_string()),
        )?;
        let (public, private) = hsm.generate_key_pair(
            &Mechanism::EccKeyPairGen,
            &ECDSA_PUB_KEY_TEMPLATE,
            &ECDSA_PRIV_KEY_TEMPLATE,
        )?;

        // TODO: This operation should be done on the HSM, but cryptoki doesn't support it yet
        // See https://github.com/parallaxsecond/rust-cryptoki/issues/88
        let digest = sha2::Sha256::digest(MSG);

        let hsm_signature = hsm.sign(digest.as_slice(), &HsmMechanism::Ecdsa)?;
        hsm.verify(digest.as_slice(), &hsm_signature, &HsmMechanism::Ecdsa)?;

        hsm.destroy(public)?;
        hsm.destroy(private)?;

        hsm.close_session();

        Ok(())
    }

    /// Test that message signing works on the HSM and that the resulting
    /// signature can be verified on the CPU using the p256 crate
    ///
    /// This test will initialize a new token and generate a new ECDSA P256 keypair.
    /// The keypair will be destroyed at the end of the test, but the token will remain initialized.
    #[test]
    fn hsm_ecdsa_sign_p256_verify() -> Result<(), ManyError> {
        let slot = init()?;

        let mut hsm = Hsm::get_instance()?;
        hsm.open_session(
            slot,
            HsmSessionType::RW, // We need to open a RW session since we're destroying the keys at the end of the test
            Some(HsmUserType::User),
            Some(USER_PIN.to_string()),
        )?;

        let (public, private) = hsm.generate_key_pair(
            &Mechanism::EccKeyPairGen,
            &ECDSA_PUB_KEY_TEMPLATE,
            &ECDSA_PRIV_KEY_TEMPLATE,
        )?;
        // TODO: This operation should be done on the HSM, but cryptoki doesn't support it yet
        // See https://github.com/parallaxsecond/rust-cryptoki/issues/88
        let digest = sha2::Sha256::digest(MSG);

        let hsm_signature = hsm.sign(digest.as_slice(), &HsmMechanism::Ecdsa)?;

        let (ec_points, _ec_params) = hsm.ec_info(HsmMechanismType::ECDSA)?;
        let points =
            p256::EncodedPoint::from_bytes(ec_points).expect("Unable to create p256::EncodedPoint");
        let verify_key = p256::ecdsa::VerifyingKey::from_encoded_point(&points).unwrap();
        let p256_signature = p256::ecdsa::Signature::try_from(hsm_signature.as_slice()).unwrap();
        verify_key
            .verify(MSG.as_bytes(), &p256_signature)
            .expect("Unable to verify signature");

        hsm.destroy(private)?;
        hsm.destroy(public)?;

        hsm.close_session();
        Ok(())
    }
}
