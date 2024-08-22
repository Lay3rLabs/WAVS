//! Makes use of logic from `wasmtime serve` command
//! https://github.com/bytecodealliance/wasmtime/blob/main/src/commands/serve.rs
mod common;
use crate::common::{RunCommon, RunTarget};
use anyhow::{anyhow, bail, Context, Error, Result};
use axum::{
    extract::{DefaultBodyLimit, Query, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum_macros::debug_handler;
use bytes::Bytes;
use clap::Parser;
use serde::Deserialize;
use std::{
    collections::HashMap,
    env,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};
use wasmtime::{
    component::{Component, Linker},
    Config, Engine, Memory, MemoryType, Store, StoreLimits,
};
use wasmtime_wasi::{StreamError, StreamResult, WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::bindings::http::types::Scheme;
use wasmtime_wasi_http::body::HyperOutgoingBody;
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::{bindings::ProxyPre, WasiHttpCtx, WasiHttpView};

struct Host {
    table: wasmtime::component::ResourceTable,
    ctx: WasiCtx,
    http: WasiHttpCtx,

    limits: StoreLimits,

    #[cfg(feature = "wasi-nn")]
    nn: Option<WasiNnCtx>,
}

impl WasiView for Host {
    fn table(&mut self) -> &mut wasmtime_wasi::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

impl WasiHttpView for Host {
    fn table(&mut self) -> &mut wasmtime::component::ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }
}

const DEFAULT_ADDR: std::net::SocketAddr = std::net::SocketAddr::new(
    std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
    8080,
);

#[derive(Parser, PartialEq)]
#[command()]
struct Wasmatic {
    #[command(subcommand)]
    subcommand: Subcommand,
}

impl Wasmatic {
    fn execute(self) -> Result<()> {
        let subcommand = self.subcommand;
        match subcommand {
            Subcommand::Wasm(c) => c.execute(),
            Subcommand::Up(c) => c.execute(),
        }
    }
}

#[derive(Parser, PartialEq)]
enum Subcommand {
    /// Serves requests for the operator API
    Wasm(WasmCommand),
    Up(UpCommand),
}

#[derive(Parser, PartialEq)]
pub struct UpCommand {
    #[command(flatten)]
    run: RunCommon,
    /// Socket address for the web server to bind to.
    #[arg(long = "addr", value_name = "SOCKADDR", default_value_t = DEFAULT_ADDR )]
    addr: SocketAddr,
}

impl UpCommand {
    pub fn execute(self) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .enable_io()
            .build()?;
        runtime.block_on(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    Ok::<_, anyhow::Error>(())
                }

                res = self.serve() => {
                    res
                }
            }
        })?;
        Ok(())
    }

    async fn serve(&self) -> Result<()> {
        let mut config = Config::new();
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config).unwrap();
        let mut builder = WasiCtxBuilder::new();
        let ctx = builder.build();
        let host = Host {
            table: wasmtime::component::ResourceTable::new(),
            ctx,
            http: WasiHttpCtx::new(),

            limits: StoreLimits::default(),
        };
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
        wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();
        let store = wasmtime::Store::new(&engine, host);
        let store = Arc::new(Mutex::new(store));
        let scheduled = HashMap::new();
        let scheduled = Arc::new(Mutex::new(scheduled));
        let sched = JobScheduler::new().await?;
        sched.start().await.unwrap();
        let addr = match env::var("WASMATIC_PORT") {
            Ok(p) => std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                p.parse()?,
            ),
            Err(_) => self.addr,
        };
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("failed to bind to address `{addr}`"))?;
        let router = Router::new()
            .route("/", get(root))
            .route("/sched", post(sched_handler))
            .route("/register", post(reg_handler))
            .layer(DefaultBodyLimit::max(1000000000000))
            .with_state((sched, engine, linker, store, scheduled));
        println!("Listening on {}", addr);
        let server = axum::serve::serve(listener, router.into_make_service());
        server.await?;

        Ok(())
    }
}

#[derive(Deserialize)]
struct Upload {
    name: String,
}
// async fn reg_handler(name: Query<Upload>, Json(bytes): Json<Vec<u8>>) -> String {
async fn reg_handler(name: Query<Upload>, bytes: Bytes) -> String {
    dbg!(&name.name);
    std::fs::write(format!("./registered/{}.wasm", name.name), bytes).unwrap();
    "Registered".to_string()
}

#[debug_handler]
async fn sched_handler(
    State((sched, engine, linker, store, scheduled)): State<(
        JobScheduler,
        Engine,
        Linker<Host>,
        Arc<Mutex<Store<Host>>>,
        Arc<Mutex<HashMap<String, Component>>>,
    )>,
    Json(payload): Json<CronJob>,
) -> String {
    println!(
        "Scheduling handler `{}` to run with schedule {}",
        payload.name, payload.cron
    );
    let wasm = std::fs::read(format!("./registered/{}.wasm", payload.name)).unwrap();
    let component = wasmtime::component::Component::new(&engine, wasm).unwrap();
    {
        let locked = &mut scheduled.lock().await;
        locked.insert(payload.name.clone(), component.clone());
        locked
    };

    sched
        .add(
            Job::new_async(payload.cron.as_str(), move |_uuid, _l| {
                Box::pin({
                    let linker = linker.clone();
                    let store = store.clone();
                    let scheduled = scheduled.clone();
                    let name = payload.name.clone();
                    async move {
                        println!("Running `{}`", &name);

                        let mut guard = store.lock().await;
                        let scheduled = scheduled.lock().await;
                        let component = scheduled.get(&name).unwrap();

                        let instance = linker
                            .instantiate_async(&mut *guard, &component)
                            .await
                            .unwrap();
                        let func = instance
                            .get_func(&mut *guard, "handler")
                            .expect("no export named `handler` exists");
                        let mut result = [wasmtime::component::Val::String("".into())];
                        func.call_async(
                            &mut *guard,
                            &[wasmtime::component::Val::String("foobar".into())],
                            &mut result,
                        )
                        .await
                        .unwrap();
                        println!("The result of running was: {:?}", result);
                    }
                })
            })
            .unwrap(),
        )
        .await
        .unwrap();
    "Scheduled!".to_string()
}

async fn root() -> &'static str {
    "Hello, World!"
}

#[derive(Deserialize)]
struct CronJob {
    name: String,
    cron: String,
}

/// Runs a WebAssembly module
#[derive(Parser, PartialEq)]
pub struct WasmCommand {
    #[command(flatten)]
    run: RunCommon,
    /// Socket address for the web server to bind to.
    #[arg(long = "addr", value_name = "SOCKADDR", default_value_t = DEFAULT_ADDR )]
    addr: SocketAddr,
    #[arg(value_name = "WASM")]
    component: Option<PathBuf>,
}

impl WasmCommand {
    pub fn execute(self) -> Result<()> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_time()
            .enable_io()
            .build()?;

        runtime.block_on(async move {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    Ok::<_, anyhow::Error>(())
                }

                res = self.serve() => {
                    res
                }
            }
        })?;
        Ok(())
    }

    fn new_store(&self, engine: &Engine, req_id: u64) -> Result<Store<Host>> {
        let mut builder = WasiCtxBuilder::new();
        self.run.configure_wasip2(&mut builder)?;

        builder.env("REQUEST_ID", req_id.to_string());

        builder.stdout(LogStream::new(
            format!("stdout [{req_id}] :: "),
            Output::Stdout,
        ));

        builder.stderr(LogStream::new(
            format!("stderr [{req_id}] :: "),
            Output::Stderr,
        ));

        let host = Host {
            table: wasmtime::component::ResourceTable::new(),
            ctx: builder.build(),
            http: WasiHttpCtx::new(),

            limits: StoreLimits::default(),
        };

        let mut store = Store::new(engine, host);

        if self.run.common.wasm.timeout.is_some() {
            store.set_epoch_deadline(u64::from(EPOCH_PRECISION) + 1);
        }

        store.data_mut().limits = self.run.store_limits();
        store.limiter(|t| &mut t.limits);

        // If fuel has been configured, we want to add the configured
        // fuel amount to this store.
        if let Some(fuel) = self.run.common.wasm.fuel {
            store.set_fuel(fuel)?;
        }

        Ok(store)
    }

    async fn serve(mut self) -> Result<()> {
        use hyper::server::conn::http1;
        let mut config = self
            .run
            .common
            .config(None, use_pooling_allocator_by_default().unwrap_or(None))?;
        config.wasm_component_model(true);
        config.async_support(true);
        let engine = Engine::new(&config)?;
        let mut linker = Linker::new(&engine);

        self.add_to_linker(&mut linker)?;
        // wasmtime_wasi::add_to_linker_async(&mut linker).unwrap();
        // wasmtime_wasi_http::add_only_http_to_linker_async(&mut linker).unwrap();

        let component = if let Some(ref component) = self.component {
            match self.run.load_module(&engine, &component)? {
                RunTarget::Core(_) => bail!("The serve command currently requires a component"),
                RunTarget::Component(c) => c,
            }
        } else {
            let path = match env::var("WASMATIC") {
              Ok(p) => p,
              Err(_) => bail!("Must either provide path to wasm component or set WASMATIC environment variable"),
          };
            let component = PathBuf::from(path);
            match self.run.load_module(&engine, &component)? {
                RunTarget::Core(_) => bail!("The serve command currently requires a component"),
                RunTarget::Component(c) => c,
            }
        };

        let instance = linker.instantiate_pre(&component)?;
        dbg!("DID PRE");
        let instance = ProxyPre::new(instance)?;

        let socket = match &self.addr {
            SocketAddr::V4(_) => tokio::net::TcpSocket::new_v4()?,
            SocketAddr::V6(_) => tokio::net::TcpSocket::new_v6()?,
        };
        socket.set_reuseaddr(!cfg!(windows))?;
        let addr = match env::var("WASMATIC_PORT") {
            Ok(p) => std::net::SocketAddr::new(
                std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)),
                p.parse()?,
            ),
            Err(_) => self.addr,
        };
        socket.bind(addr)?;
        let listener = socket.listen(100)?;

        eprintln!("Serving HTTP on http://{}/", listener.local_addr()?);

        let _epoch_thread = if let Some(timeout) = self.run.common.wasm.timeout {
            Some(EpochThread::spawn(
                timeout / EPOCH_PRECISION,
                engine.clone(),
            ))
        } else {
            None
        };

        log::info!("Listening on {}", self.addr);

        let handler = ProxyHandler::new(self, engine, instance);

        loop {
            let (stream, _) = listener.accept().await?;
            let stream = TokioIo::new(stream);
            let h = handler.clone();
            tokio::task::spawn(async {
                if let Err(e) = http1::Builder::new()
                    .keep_alive(true)
                    .serve_connection(
                        stream,
                        hyper::service::service_fn(move |req| handle_request(h.clone(), req)),
                    )
                    .await
                {
                    eprintln!("error: {e:?}");
                }
            });
        }
        Ok(())
    }

    fn add_to_linker(&self, linker: &mut Linker<Host>) -> Result<()> {
        let mut cli = self.run.common.wasi.cli;

        // Accept -Scommon as a deprecated alias for -Scli.
        if let Some(common) = self.run.common.wasi.common {
            if cli.is_some() {
                bail!(
                    "The -Scommon option should not be use with -Scli as it is a deprecated alias"
                );
            } else {
                // In the future, we may add a warning here to tell users to use
                // `-S cli` instead of `-S common`.
                cli = Some(common);
            }
        }

        // Repurpose the `-Scli` flag of `wasmtime run` for `wasmtime serve`
        // to serve as a signal to enable all WASI interfaces instead of just
        // those in the `proxy` world. If `-Scli` is present then add all
        // `command` APIs and then additionally add in the required HTTP APIs.
        //
        // If `-Scli` isn't passed then use the `add_to_linker_async`
        // bindings which adds just those interfaces that the proxy interface
        // uses.
        if cli == Some(true) {
            wasmtime_wasi::add_to_linker_async(linker)?;
            wasmtime_wasi_http::add_only_http_to_linker_async(linker)?;
        } else {
            wasmtime_wasi_http::add_to_linker_async(linker)?;
        }

        if self.run.common.wasi.nn == Some(true) {
            #[cfg(not(feature = "wasi-nn"))]
            {
                bail!("support for wasi-nn was disabled at compile time");
            }
            #[cfg(feature = "wasi-nn")]
            {
                wasmtime_wasi_nn::wit::add_to_linker(linker, |h: &mut Host| {
                    let ctx = h.nn.as_mut().unwrap();
                    wasmtime_wasi_nn::wit::WasiNnView::new(&mut h.table, ctx)
                })?;
            }
        }

        if self.run.common.wasi.threads == Some(true) {
            bail!("support for wasi-threads is not available with components");
        }

        if self.run.common.wasi.http == Some(false) {
            bail!("support for wasi-http must be enabled for `serve` subcommand");
        }

        dbg!("ADDED TO LINKER");
        Ok(())
    }
}

fn main() -> Result<()> {
    return Wasmatic::parse().execute();
}

fn use_pooling_allocator_by_default() -> Result<Option<bool>> {
    const BITS_TO_TEST: u32 = 42;
    let mut config = Config::new();
    config.wasm_memory64(true);
    config.static_memory_maximum_size(1 << BITS_TO_TEST);
    let engine = Engine::new(&config)?;
    let mut store = Store::new(&engine, ());
    // NB: the maximum size is in wasm pages to take out the 16-bits of wasm
    // page size here from the maximum size.
    let ty = MemoryType::new64(0, Some(1 << (BITS_TO_TEST - 16)));
    if Memory::new(&mut store, ty).is_ok() {
        Ok(Some(true))
    } else {
        Ok(None)
    }
}

/// This is the number of epochs that we will observe before expiring a request handler. As
/// instances may be started at any point within an epoch, and epochs are counted globally per
/// engine, we expire after `EPOCH_PRECISION + 1` epochs have been observed. This gives a maximum
/// overshoot of `timeout / EPOCH_PRECISION`, which is more desirable than expiring early.
const EPOCH_PRECISION: u32 = 10;

struct EpochThread {
    shutdown: Arc<AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl EpochThread {
    fn spawn(timeout: std::time::Duration, engine: Engine) -> Self {
        let shutdown = Arc::new(AtomicBool::new(false));
        let handle = {
            let shutdown = Arc::clone(&shutdown);
            let handle = std::thread::spawn(move || {
                while !shutdown.load(Ordering::Relaxed) {
                    std::thread::sleep(timeout);
                    engine.increment_epoch();
                }
            });
            Some(handle)
        };

        EpochThread { shutdown, handle }
    }
}

impl Drop for EpochThread {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.shutdown.store(true, Ordering::Relaxed);
            handle.join().unwrap();
        }
    }
}

struct ProxyHandlerInner {
    cmd: WasmCommand,
    engine: Engine,
    instance_pre: ProxyPre<Host>,
    next_id: AtomicU64,
}

impl ProxyHandlerInner {
    fn next_req_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }
}

#[derive(Clone)]
struct ProxyHandler(Arc<ProxyHandlerInner>);

impl ProxyHandler {
    fn new(cmd: WasmCommand, engine: Engine, instance_pre: ProxyPre<Host>) -> Self {
        Self(Arc::new(ProxyHandlerInner {
            cmd,
            engine,
            instance_pre,
            next_id: AtomicU64::from(0),
        }))
    }
}

#[derive(Clone)]
enum Output {
    Stdout,
    Stderr,
}

impl Output {
    fn write_all(&self, buf: &[u8]) -> anyhow::Result<()> {
        use std::io::Write;

        match self {
            Output::Stdout => std::io::stdout().write_all(buf),
            Output::Stderr => std::io::stderr().write_all(buf),
        }
        .map_err(|e| anyhow!(e))
    }
}

#[derive(Clone)]
struct LogStream {
    prefix: String,
    output: Output,
    needs_prefix_on_next_write: bool,
}

impl LogStream {
    fn new(prefix: String, output: Output) -> LogStream {
        LogStream {
            prefix,
            output,
            needs_prefix_on_next_write: true,
        }
    }
}

impl wasmtime_wasi::StdoutStream for LogStream {
    fn stream(&self) -> Box<dyn wasmtime_wasi::HostOutputStream> {
        Box::new(self.clone())
    }

    fn isatty(&self) -> bool {
        use std::io::IsTerminal;

        match &self.output {
            Output::Stdout => std::io::stdout().is_terminal(),
            Output::Stderr => std::io::stderr().is_terminal(),
        }
    }
}

impl wasmtime_wasi::HostOutputStream for LogStream {
    fn write(&mut self, bytes: bytes::Bytes) -> StreamResult<()> {
        let mut bytes = &bytes[..];

        while !bytes.is_empty() {
            if self.needs_prefix_on_next_write {
                self.output
                    .write_all(self.prefix.as_bytes())
                    .map_err(StreamError::LastOperationFailed)?;
                self.needs_prefix_on_next_write = false;
            }
            match bytes.iter().position(|b| *b == b'\n') {
                Some(i) => {
                    let (a, b) = bytes.split_at(i + 1);
                    bytes = b;
                    self.output
                        .write_all(a)
                        .map_err(StreamError::LastOperationFailed)?;
                    self.needs_prefix_on_next_write = true;
                }
                None => {
                    self.output
                        .write_all(bytes)
                        .map_err(StreamError::LastOperationFailed)?;
                    break;
                }
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> StreamResult<()> {
        Ok(())
    }

    fn check_write(&mut self) -> StreamResult<usize> {
        Ok(1024 * 1024)
    }
}

#[async_trait::async_trait]
impl wasmtime_wasi::Subscribe for LogStream {
    async fn ready(&mut self) {}
}

type Request = hyper::Request<hyper::body::Incoming>;

async fn handle_request(
    ProxyHandler(inner): ProxyHandler,
    req: Request,
) -> Result<hyper::Response<HyperOutgoingBody>> {
    let (sender, receiver) = tokio::sync::oneshot::channel();

    let task = tokio::task::spawn(async move {
        let req_id = inner.next_req_id();

        log::info!(
            "Request {req_id} handling {} to {}",
            req.method(),
            req.uri()
        );

        let mut store = inner.cmd.new_store(&inner.engine, req_id)?;

        let req = store.data_mut().new_incoming_request(Scheme::Http, req)?;
        let out = store.data_mut().new_response_outparam(sender)?;

        let proxy = inner.instance_pre.instantiate_async(&mut store).await?;

        if let Err(e) = proxy
            .wasi_http_incoming_handler()
            .call_handle(store, req, out)
            .await
        {
            log::error!("[{req_id}] :: {:#?}", e);
            return Err(e);
        }

        Ok(())
    });

    match receiver.await {
        Ok(Ok(resp)) => Ok(resp),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => {
            // An error in the receiver (`RecvError`) only indicates that the
            // task exited before a response was sent (i.e., the sender was
            // dropped); it does not describe the underlying cause of failure.
            // Instead we retrieve and propagate the error from inside the task
            // which should more clearly tell the user what went wrong. Note
            // that we assume the task has already exited at this point so the
            // `await` should resolve immediately.
            let e = match task.await {
                Ok(r) => r.expect_err("if the receiver has an error, the task must have failed"),
                Err(e) => e.into(),
            };
            bail!("guest never invoked `response-outparam::set` method: {e:?}")
        }
    }
}
