// Copyright 2020-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use identity_account_storage::types::CekAlgorithm;
use napi::Result;
use napi_derive::napi;

use crate::error::NapiResult;

#[napi]
pub struct NapiCekAlgorithm(pub(crate) CekAlgorithm);

#[napi]
impl NapiCekAlgorithm {
  #[napi(js_name = fromJSON)]
  pub fn from_json(json_value: serde_json::Value) -> Result<NapiCekAlgorithm> {
    serde_json::from_value(json_value).map(Self).napi_result()
  }

  #[napi(js_name = toJSON)]
  pub fn to_json(&self) -> Result<serde_json::Value> {
    serde_json::to_value(&self.0).napi_result()
  }
}