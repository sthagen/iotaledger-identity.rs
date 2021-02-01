// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Display;
use core::fmt::Error as FmtError;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use identity_core::common::Context;
use identity_core::common::Object;
use identity_core::common::OneOrMany;
use identity_core::common::Url;
use identity_core::convert::ToJson;
use serde::Serialize;

use crate::credential::Credential;
use crate::credential::Policy;
use crate::credential::Refresh;
use crate::credential::VerifiableCredential;
use crate::error::Error;
use crate::error::Result;
use crate::presentation::Builder;

/// A `Presentation` represents a bundle of one or more `VerifiableCredential`s.
///
/// `Presentation`s can be signed with `Document`s to create `VerifiablePresentation`s.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Presentation<T = Object, U = Object> {
  /// The JSON-LD context(s) applicable to the `Presentation`.
  #[serde(rename = "@context")]
  pub context: OneOrMany<Context>,
  /// A unique `URI` referencing the subject of the `Presentation`.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub id: Option<Url>,
  /// One or more URIs defining the type of the `Presentation`.
  #[serde(rename = "type")]
  pub types: OneOrMany<String>,
  /// Credential(s) expressing the claims of the `Presentation`.
  #[serde(default = "Default::default", rename = "verifiableCredential")]
  pub verifiable_credential: OneOrMany<VerifiableCredential<U>>,
  /// The entity that generated the presentation.
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder: Option<Url>,
  /// Service(s) used to refresh an expired `Presentation`.
  #[serde(default, rename = "refreshService", skip_serializing_if = "OneOrMany::is_empty")]
  pub refresh_service: OneOrMany<Refresh>,
  /// Terms-of-use specified by the `Presentation` holder.
  #[serde(default, rename = "termsOfUse", skip_serializing_if = "OneOrMany::is_empty")]
  pub terms_of_use: OneOrMany<Policy>,
  /// Miscellaneous properties.
  #[serde(flatten)]
  pub properties: T,
}

impl<T, U> Presentation<T, U> {
  /// Returns the base JSON-LD context for `Presentation`s.
  pub fn base_context() -> &'static Context {
    Credential::<U>::base_context()
  }

  /// Returns the base type for `Presentation`s.
  pub const fn base_type() -> &'static str {
    "VerifiablePresentation"
  }

  /// Creates a `Builder` to configure a new `Presentation`.
  ///
  /// This is the same as `Builder::new()`.
  pub fn builder(properties: T) -> Builder<T, U> {
    Builder::new(properties)
  }

  /// Returns a new `Presentation` based on the `Builder` configuration.
  pub fn from_builder(builder: Builder<T, U>) -> Result<Self> {
    let this: Self = Self {
      context: builder.context.into(),
      id: builder.id,
      types: builder.types.into(),
      verifiable_credential: builder.credentials.into(),
      holder: builder.holder,
      refresh_service: builder.refresh.into(),
      terms_of_use: builder.policy.into(),
      properties: builder.properties,
    };

    this.check_structure()?;

    Ok(this)
  }

  /// Validates the semantic structure of the `Presentation`.
  pub fn check_structure(&self) -> Result<()> {
    // Ensure the base context is present and in the correct location
    match self.context.get(0) {
      Some(context) if context == Self::base_context() => {}
      Some(_) | None => return Err(Error::MissingBaseContext),
    }

    // The set of types MUST contain the base type
    if !self.types.iter().any(|type_| type_ == Self::base_type()) {
      return Err(Error::MissingBaseType);
    }

    // Check all credentials.
    for credential in self.verifiable_credential.iter() {
      credential.check_structure()?;
    }

    Ok(())
  }
}

impl<T, U> Display for Presentation<T, U>
where
  T: Serialize,
  U: Serialize,
{
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    if f.alternate() {
      f.write_str(&self.to_json_pretty().map_err(|_| FmtError)?)
    } else {
      f.write_str(&self.to_json().map_err(|_| FmtError)?)
    }
  }
}

#[cfg(test)]
mod tests {
  use identity_core::convert::FromJson;

  use crate::credential::Subject;
  use crate::credential::VerifiableCredential;
  use crate::presentation::VerifiablePresentation;

  const JSON: &str = include_str!("../../tests/fixtures/presentation-1.json");

  #[test]
  #[rustfmt::skip]
  fn test_from_json() {
    let presentation: VerifiablePresentation = VerifiablePresentation::from_json(JSON).unwrap();
    let credential: &VerifiableCredential = presentation.verifiable_credential.get(0).unwrap();
    let subject: &Subject = credential.credential_subject.get(0).unwrap();

    assert_eq!(presentation.context.as_slice(), ["https://www.w3.org/2018/credentials/v1", "https://www.w3.org/2018/credentials/examples/v1"]);
    assert_eq!(presentation.id.as_ref().unwrap(), "urn:uuid:3978344f-8596-4c3a-a978-8fcaba3903c5");
    assert_eq!(presentation.types.as_slice(), ["VerifiablePresentation", "CredentialManagerPresentation"]);
    assert_eq!(presentation.proof().get(0).unwrap().type_(), "RsaSignature2018");

    assert_eq!(credential.context.as_slice(), ["https://www.w3.org/2018/credentials/v1", "https://www.w3.org/2018/credentials/examples/v1"]);
    assert_eq!(credential.id.as_ref().unwrap(), "http://example.edu/credentials/3732");
    assert_eq!(credential.types.as_slice(), ["VerifiableCredential", "UniversityDegreeCredential"]);
    assert_eq!(credential.issuer.url(), "https://example.edu/issuers/14");
    assert_eq!(credential.issuance_date, "2010-01-01T19:23:24Z".parse().unwrap());
    assert_eq!(credential.proof().get(0).unwrap().type_(), "RsaSignature2018");

    assert_eq!(subject.id.as_ref().unwrap(), "did:example:ebfeb1f712ebc6f1c276e12ec21");
    assert_eq!(subject.properties["degree"]["type"], "BachelorDegree");
    assert_eq!(subject.properties["degree"]["name"], "Bachelor of Science in Mechanical Engineering");
  }
}
