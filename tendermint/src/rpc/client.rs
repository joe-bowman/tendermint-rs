//! Tendermint RPC client

use crate::{
    abci::{self, Transaction},
    block::Height,
    net,
    rpc::{endpoint::*, Error, error::Code, Request, Response},
    Genesis,
};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request as WRequest, RequestInit as WRequestInit, RequestMode as WRequestMode, Response as WResponse};

/// Tendermint RPC client.
///
/// Presently supports JSONRPC via HTTP.
pub struct Client {
    /// Address of the RPC server
    address: net::Address,
}

impl Client {
    /// Create a new Tendermint RPC client, connecting to the given address
    pub fn new(address: net::Address) -> Self {
        Self { address }
    }

    /// `/abci_info`: get information about the ABCI application.
    pub async fn abci_info(&self) -> Result<abci_info::AbciInfo, Error> {
        Ok(self.perform(abci_info::Request).await?.response)
    }

    /// `/abci_query`: query the ABCI application
    pub async fn abci_query(
        &self,
        path: Option<abci::Path>,
        data: impl Into<Vec<u8>>,
        height: Option<Height>,
        prove: bool,
    ) -> Result<abci_query::AbciQuery, Error> {
        Ok(self
            .perform(abci_query::Request::new(path, data, height, prove))
            .await?
            .response)
    }

    /// `/block`: get block at a given height.
    pub async fn block(&self, height: impl Into<Height>) -> Result<block::Response, Error> {
        self.perform(block::Request::new(height.into())).await
    }

    /// `/block`: get the latest block.
    pub async fn latest_block(&self) -> Result<block::Response, Error> {
        self.perform(block::Request::default()).await
    }

    /// `/block_results`: get ABCI results for a block at a particular height.
    pub async fn block_results<H>(&self, height: H) -> Result<block_results::Response, Error>
    where
        H: Into<Height>,
    {
        self.perform(block_results::Request::new(height.into()))
            .await
    }

    /// `/block_results`: get ABCI results for the latest block.
    pub async fn latest_block_results(&self) -> Result<block_results::Response, Error> {
        self.perform(block_results::Request::default()).await
    }

    /// `/blockchain`: get block headers for `min` <= `height` <= `max`.
    ///
    /// Block headers are returned in descending order (highest first).
    ///
    /// Returns at most 20 items.
    pub async fn blockchain(
        &self,
        min: impl Into<Height>,
        max: impl Into<Height>,
    ) -> Result<blockchain::Response, Error> {
        // TODO(tarcieri): return errors for invalid params before making request?
        self.perform(blockchain::Request::new(min.into(), max.into()))
            .await
    }

    /// `/broadcast_tx_async`: broadcast a transaction, returning immediately.
    pub async fn broadcast_tx_async(
        &self,
        tx: Transaction,
    ) -> Result<broadcast::tx_async::Response, Error> {
        self.perform(broadcast::tx_async::Request::new(tx)).await
    }

    /// `/broadcast_tx_sync`: broadcast a transaction, returning the response
    /// from `CheckTx`.
    pub async fn broadcast_tx_sync(
        &self,
        tx: Transaction,
    ) -> Result<broadcast::tx_sync::Response, Error> {
        self.perform(broadcast::tx_sync::Request::new(tx)).await
    }

    /// `/broadcast_tx_sync`: broadcast a transaction, returning the response
    /// from `CheckTx`.
    pub async fn broadcast_tx_commit(
        &self,
        tx: Transaction,
    ) -> Result<broadcast::tx_commit::Response, Error> {
        self.perform(broadcast::tx_commit::Request::new(tx)).await
    }

    /// `/commit`: get block commit at a given height.
    pub async fn commit(&self, height: impl Into<Height>) -> Result<commit::Response, Error> {
        self.perform(commit::Request::new(height.into())).await
    }

    /// `/validators`: get validators a given height.
    pub async fn validators<H>(&self, height: H) -> Result<validators::Response, Error>
    where
        H: Into<Height>,
    {
        self.perform(validators::Request::new(height.into())).await
    }

    /// `/commit`: get the latest block commit
    pub async fn latest_commit(&self) -> Result<commit::Response, Error> {
        self.perform(commit::Request::default()).await
    }

    /// `/health`: get node health.
    ///
    /// Returns empty result (200 OK) on success, no response in case of an error.
    pub async fn health(&self) -> Result<(), Error> {
        self.perform(health::Request).await?;
        Ok(())
    }

    /// `/genesis`: get genesis file.
    pub async fn genesis(&self) -> Result<Genesis, Error> {
        Ok(self.perform(genesis::Request).await?.genesis)
    }

    /// `/net_info`: obtain information about P2P and other network connections.
    pub async fn net_info(&self) -> Result<net_info::Response, Error> {
        self.perform(net_info::Request).await
    }

    /// `/status`: get Tendermint status including node info, pubkey, latest
    /// block hash, app hash, block height and time.
    pub async fn status(&self) -> Result<status::Response, Error> {
        self.perform(status::Request).await
    }

    /// Perform a request against the RPC endpoint
    pub async fn perform<R>(&self, request: R) -> Result<R::Response, Error>
    where
        R: Request,
    {
        let request_body = request.into_json();

        let (host, port) = match &self.address {
            net::Address::Tcp { host, port, .. } => (host, port),
            other => {
                return Err(Error::invalid_params(&format!(
                    "invalid RPC address: {:?}",
                    other
                )))
            }
        };

        let mut opts = WRequestInit::new();
        opts.method("POST");
        opts.mode(WRequestMode::Cors);
        opts.body(Some(&JsValue::from_str(&request_body)));


        let request = WRequest::new_with_str_and_init(&format!("http://{}:{}/", host, port), &opts)?;

        let res = request.headers().set("Content-Type", "application/json");
        match res {
            Ok(_r) => {},
            Err(error) => return Err(Error::new(Code::Other(-1), error.as_string())),
        };
        let res = request.headers().set("User-Agent", &format!("tendermint.rs/{}", env!("CARGO_PKG_VERSION")));
        match res {
            Ok(_r) => {},
            Err(error) => return Err(Error::new(Code::Other(-1), error.as_string())),
        };

        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;

        // `resp_value` is a `Response` object.
        assert!(resp_value.is_instance_of::<WResponse>());
        let resp: WResponse = resp_value.dyn_into().unwrap();
        let json = JsFuture::from(resp.json()?).await?;

        match json.as_string() {
            None => Err(Error::parse_error("Unable to parse JSON response")),
            Some(val) => R::Response::from_string(val)
        }
    }
}
