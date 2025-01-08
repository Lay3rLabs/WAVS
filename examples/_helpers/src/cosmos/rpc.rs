// raw functions for talking to cosmos over a rpc endpoint
use crate::wasi::{Method, Request, WasiPollable};
use anyhow::{anyhow, Result};
use layer_climb_config::ChainConfig;
use tendermint_rpc::Response;
use wstd::runtime::Reactor;

pub async fn block(
    chain_config: &ChainConfig,
    reactor: &Reactor,
    height: Option<u64>,
) -> Result<tendermint_rpc::endpoint::block::v0_38::DialectResponse> {
    send(
        &reactor,
        &chain_config,
        tendermint_rpc::endpoint::block::Request {
            height: height.map(|h| h.try_into()).transpose()?,
        },
    )
    .await
}

pub async fn abci_query(
    chain_config: &ChainConfig,
    reactor: &Reactor,
    path: String,
    data: Vec<u8>,
    height: Option<u64>,
    prove: bool,
) -> Result<tendermint_rpc::endpoint::abci_query::AbciQuery> {
    let height = match height {
        Some(height) => Some(tendermint::block::Height::try_from(height)?),
        None => {
            // according to the rpc docs, 0 is latest... not sure what native None means
            Some(tendermint::block::Height::try_from(0u64)?)
        }
    };

    Ok(send(
        reactor,
        &chain_config,
        tendermint_rpc::endpoint::abci_query::Request {
            path: Some(path),
            data,
            height,
            prove,
        },
    )
    .await?
    .response)
}

pub async fn abci_protobuf_query<REQ, RESP>(
    chain_config: &ChainConfig,
    reactor: &Reactor,
    path: impl ToString,
    req: REQ,
    height: Option<u64>,
) -> Result<RESP>
where
    REQ: layer_climb_proto::Name,
    RESP: layer_climb_proto::Name + Default,
{
    let resp = abci_query(
        chain_config,
        reactor,
        path.to_string(),
        req.encode_to_vec(),
        height,
        false,
    )
    .await?;

    RESP::decode(resp.value.as_slice()).map_err(|err| anyhow::anyhow!(err))
}

async fn send<T: tendermint_rpc::Request>(
    reactor: &Reactor,
    chain_config: &ChainConfig,
    data: T,
) -> Result<T::Response> {
    let mut req = Request::new(Method::Post, &chain_config.rpc_endpoint.clone().unwrap())
        .map_err(|e| anyhow!("{:?}", e))?;

    req.body = data.into_json().as_bytes().to_vec();
    req.headers
        .push(("content-type".to_string(), "application/json".to_string()));

    let res = reactor.send(req).await.map_err(|e| anyhow!("{:?}", e))?;

    match res.status {
        200 => T::Response::from_string(res.body).map_err(|err| anyhow::anyhow!(err)),
        status => Err(anyhow!("unexpected status code: {status}")),
    }
}
