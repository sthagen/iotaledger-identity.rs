// Copyright 2020-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use identity_core::common::KeyComparable;
use identity_data_integrity::verification_material::Multikey;
use identity_data_integrity::verification_material::PublicKeyMultibase;
use identity_data_integrity::verification_material::VerificationMaterial;
use identity_did::did::DIDUrl;
use identity_did::did::DID;
use identity_did::document::CoreDocument;
use identity_did::verification::MethodBuilder;
use identity_did::verification::MethodData;
use identity_did::verification::MethodScope;
use identity_did::verification::MethodType;
use identity_did::verification::VerificationMethod;

use crate::identifiers::MethodIdx;
use crate::identity_storage::IdentityStorage;
use crate::identity_storage::IdentityStorageErrorKindSplit;
use crate::key_generation::MultikeyOutput;
use crate::key_generation::MultikeySchema;
use crate::key_storage::KeyStorage;
use crate::key_storage::KeyStorageErrorKindSplit;
use crate::storage::Storage;

use super::method_creation_error::MethodCreationError;
use super::method_removal_error::MethodRemovalError;
use super::storage_error::StorageError;
use super::MethodCreationErrorKind;
use super::MethodRemovalErrorKind;

#[async_trait(?Send)]
/// Extension trait enabling [`CoreDocument`] to utilize
/// key material secured by a [`Storage`](crate::storage::Storage).
pub trait CoreDocumentExt: private::Sealed {
  type D: DID + KeyComparable;
  /// Create a new verification method of type `Multikey`
  /// whose key material is backed by a [`Storage`](crate::storage::Storage).  
  ///
  /// The `schema` parameter declares what kind of key you wish the
  /// storage to generate, while `fragment` defines the fragment of the created method's relative DIDUrl and `scope`
  /// declares what purpose the created method can be used for. See the section on verification relationships in [the data integrity specification](https://w3c.github.io/vc-data-integrity/#verification-relationships).
  async fn create_multikey_method<K, I>(
    &mut self,
    fragment: &str,
    schema: &MultikeySchema,
    storage: &Storage<K, I>,
    scope: MethodScope,
  ) -> Result<(), MethodCreationError>
  where
    K: KeyStorage,
    I: IdentityStorage;

  /// Remove the method from the document and delete
  /// the corresponding keys and metadata from the [`Storage`](crate::storage::Storage).
  ///
  /// # Warning
  ///
  /// This operation cannot be undone.
  ///
  /// # Behaviour
  ///
  /// If the key material corresponding to the verification method is successfully removed from the underlying
  /// [`KeyStorage`] the verification method will also be removed from the document. There is still a non-zero chance
  /// that the [`IdentityStorage`] fails to delete the metadata of the verification method in which case an error is
  /// returned. In other words the absence of an `Ok`, does not necessarily mean that the method still exists in the
  /// document.
  async fn purge_method<K, I>(
    &mut self,
    did_url: &DIDUrl<Self::D>,
    storage: &Storage<K, I>,
  ) -> Result<(), MethodRemovalError>
  where
    K: KeyStorage,
    I: IdentityStorage;
}

mod private {
  use identity_core::common::KeyComparable;
  use identity_did::did::DID;
  use identity_did::document::CoreDocument;

  pub trait Sealed {}

  impl<D, T, U, V> Sealed for CoreDocument<D, T, U, V> where D: DID + KeyComparable {}
}

#[async_trait(?Send)]
impl<D: DID + KeyComparable, T, U, V> CoreDocumentExt for CoreDocument<D, T, U, V>
where
  U: Default,
{
  type D = D;
  async fn create_multikey_method<K, I>(
    &mut self,
    fragment: &str,
    schema: &MultikeySchema,
    storage: &Storage<K, I>,
    scope: MethodScope,
  ) -> Result<(), MethodCreationError>
  where
    K: KeyStorage,
    I: IdentityStorage,
  {
    // Check if the fragment already exists
    if self.refers_to_sub_resource(fragment) {
      return Err(MethodCreationError::from_kind(MethodCreationErrorKind::FragmentInUse));
    }

    let did_url = {
      let mut did_url = self.id().to_url();
      did_url
        .set_fragment(Some(&fragment))
        .map_err(|_| MethodCreationError::from_kind(MethodCreationErrorKind::InvalidFragmentSyntax))?;
      did_url
    };

    // Use the key storage to generate a multikey:

    let MultikeyOutput { public_key, key_id } = match storage.key_storage().generate_multikey(schema).await {
      Ok(output) => output,
      Err(key_storage_error) => {
        let error_kind: MethodCreationErrorKind = match key_storage_error.kind().split() {
          KeyStorageErrorKindSplit::Common(common) => common.into(),
          KeyStorageErrorKindSplit::UnsupportedMultikeySchema => MethodCreationErrorKind::UnsupportedMultikeySchema,
          // The other variants should not be relevant for this operation
          KeyStorageErrorKindSplit::KeyNotFound | KeyStorageErrorKindSplit::UnsupportedSigningKey => {
            MethodCreationErrorKind::UnspecifiedStorageFailure
          }
        };
        return Err(MethodCreationError::new(
          error_kind,
          StorageError::KeyStorage(key_storage_error),
        ));
      }
    };

    let method_idx: MethodIdx = MethodIdx::new_from_multikey(fragment, &public_key);

    if let Err(identity_storage_error) = storage.identity_storage().store_key_id(method_idx, &key_id).await {
      // Attempt to rollback key generation
      if let Err(key_storage_error) = storage.key_storage().delete(&key_id).await {
        let error_kind: MethodCreationErrorKind = MethodCreationErrorKind::TransactionRollbackFailure;
        return Err(MethodCreationError::new(
          error_kind,
          StorageError::Both(Box::new((key_storage_error, identity_storage_error))),
        ));
      } else {
        // Rollback succeeded, only need to report the identity storage error
        let error_kind = match identity_storage_error.kind().split() {
          IdentityStorageErrorKindSplit::Common(common) => common.into(),
          IdentityStorageErrorKindSplit::MethodIdxAlreadyExists => MethodCreationErrorKind::MethodMetadataAlreadyStored,
          // The other variants should not be relevant for this operation
          IdentityStorageErrorKindSplit::MethodIdxNotFound => MethodCreationErrorKind::UnspecifiedStorageFailure,
        };
        return Err(MethodCreationError::new(
          error_kind,
          StorageError::IdentityStorage(identity_storage_error),
        ));
      }
    }

    let method = MethodBuilder::<D, U>::default()
      .id(did_url)
      .type_(MethodType::MULTIKEY)
      // This line should be removed once we replace MethodData with VerificationMaterial
      .data(MethodData::PublicKeyMultibase(public_key.as_multibase_str().to_owned()))
      .material(VerificationMaterial::PublicKeyMultibase(PublicKeyMultibase::new(
        public_key.into_multibase_string(),
      )))
      .build()
      .expect("building a method with valid data should be fine");

    self
      .insert_method(method, scope)
      .map_err(|_| MethodCreationError::from_kind(MethodCreationErrorKind::FragmentInUse))
  }

  async fn purge_method<K, I>(&mut self, did_url: &DIDUrl<D>, storage: &Storage<K, I>) -> Result<(), MethodRemovalError>
  where
    K: KeyStorage,
    I: IdentityStorage,
  {
    // TODO: What to do about the VerificationMaterial::Multikey variant?
    let public_key_multibase = self
      .resolve_method(did_url, None)
      .and_then(VerificationMethod::material)
      .and_then(|material| match material {
        VerificationMaterial::PublicKeyMultibase(ref public_key_multibase) => Some(public_key_multibase),
        _ => None,
      })
      .ok_or_else(|| MethodRemovalError::from_kind(MethodRemovalErrorKind::MethodNotFound))?;

    let method_idx = MethodIdx::new_from_multikey(
      did_url.fragment().unwrap_or_default(),
      &Multikey::from_multibase_string(public_key_multibase.as_str().to_owned()),
    );

    match storage.identity_storage().get_key_id(&method_idx).await {
      Ok(key_id) => {
        if let Err(key_storage_error) = storage.key_storage().delete(&key_id).await {
          let error_kind = match key_storage_error.kind().split() {
            KeyStorageErrorKindSplit::Common(common) => common.into(),
            KeyStorageErrorKindSplit::KeyNotFound => MethodRemovalErrorKind::KeyNotFound,
            // Other variants are irrelevant
            KeyStorageErrorKindSplit::UnsupportedMultikeySchema | KeyStorageErrorKindSplit::UnsupportedSigningKey => {
              MethodRemovalErrorKind::UnspecifiedStorageFailure
            }
          };
          return Err(MethodRemovalError::new(error_kind, key_storage_error.into()));
        } else {
          // The key material has been removed
          let key_id_removal_result =
            storage
              .identity_storage()
              .delete_key_id(method_idx)
              .await
              .map_err(|identity_storage_error| {
                let error_kind: MethodRemovalErrorKind = match identity_storage_error.kind().split() {
                  IdentityStorageErrorKindSplit::Common(common) => common.into(),
                  // It is very unlikely for this to happen as we found an entry under this id previously when looking
                  // up the key_id
                  IdentityStorageErrorKindSplit::MethodIdxNotFound => MethodRemovalErrorKind::UnspecifiedStorageFailure,
                  // This variant is irrelevant for this operation
                  IdentityStorageErrorKindSplit::MethodIdxAlreadyExists => {
                    MethodRemovalErrorKind::UnspecifiedStorageFailure
                  }
                };
                MethodRemovalError::new(error_kind, identity_storage_error.into())
              });
          // Proceed with removing the method regardless of whether the `key_id` was purged.
          self
            .remove_method(did_url)
            .ok_or_else(|| MethodRemovalError::from_kind(MethodRemovalErrorKind::MethodNotFound))?;
          key_id_removal_result
        }
      }
      Err(identity_storage_error) => {
        let error_kind = match identity_storage_error.kind().split() {
          IdentityStorageErrorKindSplit::Common(common) => common.into(),
          IdentityStorageErrorKindSplit::MethodIdxNotFound => MethodRemovalErrorKind::MethodMetadataNotFound,
          // Other variants are irrelevant
          IdentityStorageErrorKindSplit::MethodIdxAlreadyExists => MethodRemovalErrorKind::UnspecifiedStorageFailure,
        };
        Err(MethodRemovalError::new(error_kind, identity_storage_error.into()))
      }
    }
  }
}