use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use pink_types::js::JsValue;
use scale::Decode;
use tracing::{error, info};
use wapo_host::{wasmtime::Config, InstanceConfig, Meter, WasmEngine};

/// The compiler backend to use
#[derive(ValueEnum, Clone, Debug)]
enum Compiler {
    Auto,
    Cranelift,
    Winch,
}

impl From<Compiler> for wapo_host::wasmtime::Strategy {
    fn from(compiler: Compiler) -> Self {
        match compiler {
            Compiler::Auto => Self::Auto,
            Compiler::Cranelift => Self::Cranelift,
            Compiler::Winch => Self::Winch,
        }
    }
}

#[derive(Parser, Debug)]
#[clap(about = "wapo runner", version, author)]
pub struct Args {
    /// Max memory pages
    #[arg(long, short = 'M', default_value_t = 256)]
    max_memory_pages: u32,
    /// Decode the Output as JsValue
    #[arg(long, short = 'j')]
    decode_js_value: bool,
    /// The epoch timeout
    #[arg(long, short = 'T')]
    kill_timeout: Option<u64>,
    /// The time of a single epoch tick
    #[arg(long, default_value_t = 10)]
    tick_time_ms: u64,
    /// The number of ticks of epoch deadline
    #[arg(long, default_value_t = 20)]
    epoch_deadline: u64,
    /// The compiler to use
    #[arg(long, short = 'c', default_value = "auto")]
    compiler: Compiler,
    /// Max memory pages
    #[arg(long = "env", short = 'E')]
    envs: Vec<String>,
    /// The WASM program to run
    program: String,
    /// The rest of the arguments are passed to the WASM program
    #[arg(
        last = true,
        trailing_var_arg = true,
        allow_hyphen_values = true,
        hide = true
    )]
    args: Vec<String>,
}

pub async fn run(mut args: Args) -> Result<(Vec<u8>, Arc<Meter>)> {
    let code = tokio::fs::read(&args.program).await?;
    let mut engine_config = Config::new();
    engine_config.strategy(args.compiler.into());
    let engine = WasmEngine::new(
        engine_config,
        args.tick_time_ms,
        args.max_memory_pages.saturating_mul(64 * 1024) as _,
        0,
    )
    .context("failed to create Wasm engine")?;
    let t0 = std::time::Instant::now();
    info!(target: "wapo", "compiling wasm module");
    let module = engine.compile(&code)?;
    info!(target: "wapo", "compiled wasm module in {:?}", t0.elapsed());
    args.args.insert(0, args.program);
    let vm_envs = args
        .envs
        .into_iter()
        .map(|s| -> Result<(String, String)> {
            let mut parts = s.splitn(2, '=');
            let key = parts.next().context("invalid env")?;
            let value = parts.next().unwrap_or_default();
            Ok((key.to_string(), value.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    let vm_args = args
        .args
        .into_iter()
        .map(|s| -> Result<String> {
            if let Some(s) = s.strip_prefix('@') {
                let content = std::fs::read_to_string(s).context("failed to read file")?;
                Ok(content)
            } else {
                Ok(s)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let config = InstanceConfig::builder()
        .epoch_deadline(args.epoch_deadline)
        .max_memory_pages(args.max_memory_pages)
        .args(vm_args)
        .envs(vm_envs)
        .blobs_dir("./data/storage_files/blobs".into())
        .runtime_calls(())
        .tcp_listen_port_range(0..=65535)
        .build();
    let mut wasm_run = module.run(config).context("failed to start the instance")?;
    if let Some(kill_timeout) = args.kill_timeout {
        let meter = wasm_run.meter().clone();
        info!(target: "wapo", "setting kill timeout to {}s", kill_timeout);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(kill_timeout));
            meter.stop();
        });
    }
    if let Err(err) = (&mut wasm_run).await {
        error!(target: "wapo", ?err, "JS runtime exited with error.");
    }
    Ok((vec![], wasm_run.meter()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    let decode_output_js = args.decode_js_value;
    let (output, meter) = run(args).await?;
    if decode_output_js {
        let js_value = JsValue::decode(&mut &output[..]).context("failed to decode JsValue")?;
        println!("Output: {:?}", js_value);
    } else {
        println!("Output: {:?}", output);
    }
    println!("Meter: {:#?}", meter);
    Ok(())
}
