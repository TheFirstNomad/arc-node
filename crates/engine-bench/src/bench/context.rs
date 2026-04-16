// Copyright 2026 Circle Internet Group, Inc. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::cli::CommonArgs;
use arc_eth_engine::rpc::{engine_rpc::EngineRpc, ethereum_rpc::EthereumRPC};
use chrono::Utc;
use eyre::{bail, Context};
use reqwest::Url;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

const ETH_BATCH_TIMEOUT_FLOOR: Duration = Duration::from_secs(30);

pub(crate) struct BenchContext {
    engine_rpc_url: String,
    output_dir: PathBuf,
    jwt_secret_path: PathBuf,
    eth_rpc_timeout: Duration,
}

impl BenchContext {
    pub(crate) fn new(common: &CommonArgs, mode: &str) -> eyre::Result<Self> {
        Ok(Self {
            engine_rpc_url: common.engine_rpc_url.clone(),
            output_dir: resolve_output_dir(common, mode)?,
            jwt_secret_path: resolve_jwt_secret_path(common)?,
            eth_rpc_timeout: Duration::from_millis(common.eth_rpc_timeout_ms),
        })
    }

    pub(crate) fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    pub(crate) fn engine_rpc(&self) -> eyre::Result<EngineRpc> {
        EngineRpc::new(
            Url::parse(&self.engine_rpc_url).wrap_err("invalid target engine rpc url")?,
            self.jwt_secret_path.as_path(),
        )
        .wrap_err("failed to create engine rpc client")
    }

    pub(crate) fn ethereum_rpc(&self, rpc_url: &str, role: &str) -> eyre::Result<EthereumRPC> {
        ethereum_rpc_client(rpc_url, role, self.eth_rpc_timeout)
    }
}

pub(crate) fn ethereum_rpc_client(
    rpc_url: &str,
    role: &str,
    eth_rpc_timeout: Duration,
) -> eyre::Result<EthereumRPC> {
    EthereumRPC::new_with_timeouts(
        Url::parse(rpc_url).wrap_err_with(|| format!("invalid {role} url"))?,
        eth_rpc_timeout,
        eth_rpc_timeout.max(ETH_BATCH_TIMEOUT_FLOOR),
    )
    .wrap_err_with(|| format!("failed to create {role} client"))
}

fn resolve_output_dir(common: &CommonArgs, mode: &str) -> eyre::Result<PathBuf> {
    let output = match &common.output {
        Some(path) => path.clone(),
        None => {
            let timestamp = Utc::now().format("%Y%m%dT%H%M%SZ");
            PathBuf::from("target")
                .join("engine-bench")
                .join(format!("{mode}-{timestamp}"))
        }
    };
    fs::create_dir_all(&output)
        .wrap_err_with(|| format!("failed to create benchmark output dir {}", output.display()))?;
    Ok(output)
}

fn resolve_jwt_secret_path(common: &CommonArgs) -> eyre::Result<PathBuf> {
    resolve_jwt_secret_path_from_arg(&common.jwt_secret)
}

fn resolve_jwt_secret_path_from_arg(path: &Path) -> eyre::Result<PathBuf> {
    if !path.exists() {
        bail!("JWT secret file does not exist: {}", path.display());
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn common_args() -> CommonArgs {
        CommonArgs {
            engine_rpc_url: "http://127.0.0.1:8551".to_string(),
            jwt_secret: PathBuf::from("jwt.hex"),
            eth_rpc_timeout_ms: 10_000,
            output: None,
        }
    }

    #[test]
    fn resolve_output_dir_creates_explicit_output_directory() {
        let temp_dir = TempDir::new().unwrap();
        let output_dir = temp_dir.path().join("bench-output");
        let mut args = common_args();
        args.output = Some(output_dir.clone());

        let resolved = resolve_output_dir(&args, "new-payload-fcu").unwrap();

        assert_eq!(resolved, output_dir);
        assert!(resolved.is_dir());
    }

    #[test]
    fn resolve_jwt_secret_path_prefers_explicit_path() {
        let temp_dir = TempDir::new().unwrap();
        let jwt_path = temp_dir.path().join("jwt.hex");
        fs::write(&jwt_path, "secret").unwrap();

        let resolved = resolve_jwt_secret_path_from_arg(&jwt_path).unwrap();

        assert_eq!(resolved, jwt_path);
    }

    #[test]
    fn resolve_jwt_secret_path_errors_when_file_is_missing() {
        let temp_dir = TempDir::new().unwrap();
        let jwt_path = temp_dir.path().join("missing.jwt");

        let err = resolve_jwt_secret_path_from_arg(&jwt_path).unwrap_err();

        assert_eq!(
            err.to_string(),
            format!("JWT secret file does not exist: {}", jwt_path.display())
        );
    }
}
