use crate::module::LedgerModuleImpl;
use coset::{CborSerializable, CoseKey};
use many_error::ManyError;
use many_identity::Address;
use many_modules::idstore;

/// Return a recall phrase
//
/// The following relation need to hold for having a valid decoding/encoding:
///
///     // length_bytes(data) * 8 + checksum = number_of(words) * 11
///
/// See [bip39-dict](https://github.com/vincenthz/bip39-dict) for details
///
/// # Generic Arguments
///
/// * `W` - Word cound
/// * `FB` - Full Bytes
/// * `CS` - Checksum Bytes
pub fn generate_recall_phrase<const W: usize, const FB: usize, const CS: usize>(
    seed: &[u8],
) -> Result<Vec<String>, ManyError> {
    let entropy = bip39_dict::Entropy::<FB>::from_slice(seed)
        .ok_or_else(|| ManyError::unknown("Unable to generate entropy"))?;
    let mnemonic = entropy.to_mnemonics::<W, CS>().unwrap();
    let recall_phrase = mnemonic
        .to_string(&bip39_dict::ENGLISH)
        .split_whitespace()
        .map(|e| e.to_string())
        .collect::<Vec<String>>();
    Ok(recall_phrase)
}

impl idstore::IdStoreModuleBackend for LedgerModuleImpl {
    fn store(
        &mut self,
        sender: &Address,
        idstore::StoreArgs {
            address,
            cred_id,
            public_key,
        }: idstore::StoreArgs,
    ) -> Result<idstore::StoreReturns, ManyError> {
        if sender.is_anonymous() {
            return Err(ManyError::invalid_identity());
        }

        if !address.is_public_key() {
            return Err(idstore::invalid_address(address.to_string()));
        }

        if !(16..=1023).contains(&cred_id.0.len()) {
            return Err(idstore::invalid_credential_id(hex::encode(&*cred_id.0)));
        }

        let _: CoseKey =
            CoseKey::from_slice(&public_key.0).map_err(ManyError::deserialization_error)?;

        let mut current_try = 1u8;
        let recall_phrase = loop {
            if current_try > 8 {
                return Err(idstore::recall_phrase_generation_failed());
            }

            let seed = self.storage.inc_idstore_seed()?;
            // Entropy can only be generated if the seed array contains the
            // EXACT amount of full bytes, i.e., the FB parameter of
            // `generate_recall_phrase`
            let recall_phrase = match seed {
                0..=0xFFFF => generate_recall_phrase::<2, 2, 6>(&seed.to_be_bytes()[6..]),
                0x10000..=0xFFFFFF => generate_recall_phrase::<3, 4, 1>(&seed.to_be_bytes()[4..]),
                0x1000000..=0xFFFFFFFF => {
                    generate_recall_phrase::<4, 5, 4>(&seed.to_be_bytes()[3..])
                }
                0x100000000..=0xFFFFFFFFFF => {
                    generate_recall_phrase::<5, 6, 7>(&seed.to_be_bytes()[2..])
                }
                _ => unimplemented!(),
            }?;

            if self.storage.get_from_recall_phrase(&recall_phrase).is_ok() {
                current_try += 1;
                tracing::debug!("Recall phrase generation failed, retrying...")
            } else {
                break recall_phrase;
            }
        };

        self.storage
            .store(&recall_phrase, &address, cred_id, public_key)?;
        Ok(idstore::StoreReturns(recall_phrase))
    }

    fn get_from_recall_phrase(
        &self,
        args: idstore::GetFromRecallPhraseArgs,
    ) -> Result<idstore::GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_recall_phrase(&args.0)?;
        Ok(idstore::GetReturns {
            cred_id,
            public_key,
        })
    }

    fn get_from_address(
        &self,
        args: idstore::GetFromAddressArgs,
    ) -> Result<idstore::GetReturns, ManyError> {
        let (cred_id, public_key) = self.storage.get_from_address(&args.0)?;
        Ok(idstore::GetReturns {
            cred_id,
            public_key,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::json::InitialStateJson;
    use crate::module::LedgerModuleImpl;
    use coset::CborSerializable;
    use many_identity::Identity;
    use many_identity_dsa::ed25519::generate_random_ed25519_identity;
    use many_modules::idstore;
    use many_modules::idstore::IdStoreModuleBackend;

    #[test]
    /// Test every recall phrase generation codepath
    fn idstore_generate_recall_phrase_all_codepaths() {
        let cose_key_id = generate_random_ed25519_identity();
        let public_key: idstore::PublicKey =
            idstore::PublicKey(cose_key_id.public_key().to_vec().unwrap().into());
        let mut module_impl = LedgerModuleImpl::new(
            InitialStateJson::read("../../staging/ledger_state.json5")
                .or_else(|_| InitialStateJson::read("staging/ledger_state.json5"))
                .expect("Could not read initial state."),
            None,
            tempfile::tempdir().unwrap(),
            false,
        )
        .unwrap();
        let cred_id = idstore::CredentialId(vec![1; 16].into());
        let id = cose_key_id.address();

        // Basic call
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 2);

        // Make sure another call provides a different result
        let result2 = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result2.is_ok());
        let rp2 = result2.unwrap().0;
        assert_eq!(rp2.len(), 2);
        assert_ne!(rp, rp2);

        // Generate the first 8 recall phrase
        for _ in 2..8 {
            let result3 = module_impl.store(
                &id,
                idstore::StoreArgs {
                    address: id,
                    cred_id: cred_id.clone(),
                    public_key: public_key.clone(),
                },
            );
            assert!(result3.is_ok());
        }

        // And reset the seed 0
        module_impl
            .storage
            .set_idstore_seed(0)
            .expect("Unable to set idstore seed.");

        // This should trigger the `recall_phrase_generation_failed()` exception
        let result4 = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result4.is_err());
        assert_eq!(
            result4.unwrap_err().code(),
            idstore::recall_phrase_generation_failed().code()
        );

        // Generate a 3-words phrase
        module_impl
            .storage
            .set_idstore_seed(0x10000)
            .expect("Unable to set idstore seed.");
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 3);

        // Generate a 4-words phrase
        module_impl
            .storage
            .set_idstore_seed(0x1000000)
            .expect("Unable to set idstore seed.");
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id: cred_id.clone(),
                public_key: public_key.clone(),
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 4);

        // Generate a 5-words phrase
        module_impl
            .storage
            .set_idstore_seed(0x100000000)
            .expect("Unable to set idstore seed.");
        let result = module_impl.store(
            &id,
            idstore::StoreArgs {
                address: id,
                cred_id,
                public_key,
            },
        );
        assert!(result.is_ok());
        let rp = result.unwrap().0;
        assert_eq!(rp.len(), 5);
    }
}
