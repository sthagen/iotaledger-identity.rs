// Copyright 2020-2023 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example shows how to create a Verifiable Credential and validate it.
//! In this example, alice takes the role of the subject, while we also have an issuer.
//! The issuer signs a UniversityDegreeCredential type verifiable credential with Alice's name and DID.
//! This Verifiable Credential can be verified by anyone, allowing Alice to take control of it and share it with
//! whomever they please.
//!
//! cargo run --release --example 5_create_vc

use anyhow::anyhow;
use examples::create_did;
use examples::MemStorage;
use identity_eddsa_verifier::EdDSAJwsVerifier;
use identity_iota::credential::Jwt;
use identity_iota::credential::JwtCredential;
use identity_iota::credential::ValidableCredential;
use identity_iota::resolver::IotaResolver;
use identity_iota::storage::JwkDocumentExt;
use identity_iota::storage::JwkMemStore;
use identity_iota::storage::JwsSignatureOptions;
use identity_iota::storage::KeyIdMemstore;
use iota_sdk::client::secret::stronghold::StrongholdSecretManager;
use iota_sdk::client::secret::SecretManager;
use iota_sdk::client::Client;
use iota_sdk::client::Password;
use iota_sdk::types::block::address::Address;

use examples::random_stronghold_path;
use examples::API_ENDPOINT;
use identity_iota::core::json;
use identity_iota::core::FromJson;
use identity_iota::core::Url;
use identity_iota::credential::Credential;
use identity_iota::credential::CredentialBuilder;
use identity_iota::credential::Subject;
use identity_iota::did::DID;
use identity_iota::iota::IotaDocument;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Create a new client to interact with the IOTA ledger.
  let client: Client = Client::builder()
    .with_primary_node(API_ENDPOINT, None)?
    .finish()
    .await?;

  // Create an identity for the issuer with one verification method `key-1`.
  let mut secret_manager_issuer: SecretManager = SecretManager::Stronghold(
    StrongholdSecretManager::builder()
      .password(Password::from("secure_password_1".to_owned()))
      .build(random_stronghold_path())?,
  );
  let issuer_storage: MemStorage = MemStorage::new(JwkMemStore::new(), KeyIdMemstore::new());
  let (_, issuer_document, fragment): (Address, IotaDocument, String) =
    create_did(&client, &mut secret_manager_issuer, &issuer_storage).await?;

  // Create an identity for the holder, in this case also the subject.
  let mut secret_manager_alice: SecretManager = SecretManager::Stronghold(
    StrongholdSecretManager::builder()
      .password(Password::from("secure_password_2".to_owned()))
      .build(random_stronghold_path())?,
  );
  let alice_storage: MemStorage = MemStorage::new(JwkMemStore::new(), KeyIdMemstore::new());
  let (_, alice_document, _): (Address, IotaDocument, String) =
    create_did(&client, &mut secret_manager_alice, &alice_storage).await?;

  // Create a credential subject indicating the degree earned by Alice.
  let subject: Subject = Subject::from_json_value(json!({
    "id": alice_document.id().as_str(),
    "name": "Alice",
    "degree": {
      "type": "BachelorDegree",
      "name": "Bachelor of Science and Arts",
    },
    "GPA": "4.0",
  }))?;

  // Build credential using subject above and issuer.
  let credential: Credential = CredentialBuilder::default()
    .id(Url::parse("https://example.edu/credentials/3732")?)
    .issuer(Url::parse(issuer_document.id().as_str())?)
    .type_("UniversityDegreeCredential")
    .subject(subject)
    .build()?;

  let credential_jwt: Jwt = issuer_document
    .create_credential_jwt(
      &credential,
      &issuer_storage,
      &fragment,
      &JwsSignatureOptions::default(),
      None,
    )
    .await?;

  let credential_jwt = JwtCredential::<Credential>::parse(credential_jwt)?;
  credential_jwt.validate(&IotaResolver::new(client), &EdDSAJwsVerifier::default()).await.map_err(|_| anyhow!("oops"))?;
  println!("{}", serde_json::to_string(&credential_jwt).unwrap());

  println!("VC successfully validated");

  println!("Credential JSON > {:#}", credential_jwt.as_ref());

  Ok(())
}
