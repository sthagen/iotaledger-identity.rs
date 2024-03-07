// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
#![doc = include_str!("./../README.md")]
#![allow(clippy::upper_case_acronyms)]
#![warn(
  rust_2018_idioms,
  unreachable_pub,
  missing_docs,
  rustdoc::missing_crate_level_docs,
  rustdoc::broken_intra_doc_links,
  rustdoc::private_intra_doc_links,
  rustdoc::private_doc_tests,
  clippy::missing_safety_doc,
  // clippy::missing_errors_doc
)]

#[doc(inline)]
pub use serde_json::json;

pub mod common;
pub mod convert;
pub mod error;

pub use self::error::Error;
pub use self::error::Result;

pub trait ResolverT<T> {
  type Error;
  type Input;

  async fn fetch(&self, input: &Self::Input) -> std::result::Result<T, Self::Error>;
}