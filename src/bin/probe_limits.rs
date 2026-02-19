//! Probe the actual context window and output token limits for each model
//! supported by the Kiro gateway.
//!
//! # Usage
//! ```bash
//! # Probe all models
//! cargo run --bin probe_limits --release
//!
//! # Probe a single model
//! cargo run --bin probe_limits --release -- --model claude-sonnet-4.6
//! ```
//!
//! Requires the gateway to be running locally. Configure via env vars:
//! - `GATEWAY_URL` - Gateway base URL (default: http://127.0.0.1:8000)
//! - `PROXY_API_KEY` - Gateway API key (required)

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

#[derive(Debug)]
enum Mode {
    SingleModel(String),
    AllModels,
}

impl Mode {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--model" => {
                    let model = args.next().context("--model requires a model ID")?;
                    return Ok(Self::SingleModel(model));
                }
                "--all-models" => return Ok(Self::AllModels),
                other => anyhow::bail!("Unknown argument: {other}"),
            }
        }
        anyhow::bail!(
            "Usage:\n  probe_limits --model <model-id>\n  probe_limits --all-models"
        )
    }
}

const DEFAULT_GATEWAY_URL: &str = "http://127.0.0.1:8000";
const PROBE_CONTENT: &str = "hello world ";
const OUTPUT_PROBE_PROMPT: &str =
    "Write a detailed essay about the history of computing. Be thorough and verbose.";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let mode = Mode::parse()?;
    let gateway_url =
        std::env::var("GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY_URL.to_string());
    let api_key = std::env::var("PROXY_API_KEY").context("PROXY_API_KEY env var required")?;

    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()?;

    println!("Gateway: {gateway_url}");

    let models = match mode {
        Mode::SingleModel(model) => {
            println!("Probing model: {model}\n");
            vec![model]
        }
        Mode::AllModels => {
            println!("Fetching supported models...\n");
            let all = fetch_models(&client, &gateway_url, &api_key).await?;
            println!("Models to probe: {}\n", all.join(", "));
            all
        }
    };

    println!(
        "{:<30} {:>16} {:>16}",
        "Model", "Context (tokens)", "Output cap"
    );
    println!("{}", "-".repeat(66));

    for model in &models {
        let context_limit = probe_context_window(&client, &gateway_url, &api_key, model).await;
        let output_limit = probe_output_tokens(&client, &gateway_url, &api_key, model).await;

        let ctx_str = match context_limit {
            Ok(n) => format!("~{n}"),
            Err(e) => format!("err: {e}"),
        };
        let out_str = match output_limit {
            Ok(Some(n)) => format!("~{n} (length)"),
            Ok(None) => "model stops early".to_string(),
            Err(e) => format!("err: {e}"),
        };

        println!("{model:<30} {ctx_str:>16} {out_str:>16}");
    }

    println!("\nDone. Use context values for `contextLength` in your OpenCode provider config.");
    Ok(())
}

/// Fetch all model IDs from the gateway's /v1/models endpoint.
async fn fetch_models(client: &Client, base_url: &str, api_key: &str) -> Result<Vec<String>> {
    let resp = client
        .get(format!("{base_url}/v1/models"))
        .bearer_auth(api_key)
        .send()
        .await?
        .json::<Value>()
        .await?;

    let models = resp["data"]
        .as_array()
        .context("Expected data array in /v1/models response")?
        .iter()
        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
        .filter(|id| id.starts_with("claude-"))
        .collect();

    Ok(models)
}

/// Binary search for the context window limit.
/// Sends increasing payloads and reads `prompt_tokens` from the usage metadata.
/// Returns the highest `prompt_tokens` value that succeeded.
async fn probe_context_window(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<u64> {
    let mut lo_chars: usize = 100_000;
    let mut hi_chars: usize = 5_000_000;
    // First check if even lo_chars works
    let mut last_good_tokens: u64 = match send_probe(client, base_url, api_key, model, lo_chars).await {
        Ok(usage) => usage.prompt_tokens,
        Err(_) => return Err(anyhow::anyhow!("model unavailable")),
    };

    // Check if hi_chars fails (if not, context window is very large)
    if send_probe(client, base_url, api_key, model, hi_chars)
        .await
        .is_ok()
    {
        return Ok(last_good_tokens); // Can't find ceiling, report last known
    }

    // Binary search
    while hi_chars - lo_chars > 20_000 {
        let mid = (lo_chars + hi_chars) / 2;
        match send_probe(client, base_url, api_key, model, mid).await {
            Ok(usage) => {
                last_good_tokens = usage.prompt_tokens;
                lo_chars = mid;
            }
            Err(_) => {
                hi_chars = mid;
            }
        }
    }

    Ok(last_good_tokens)
}

/// Try to find the output token cap by forcing `finish_reason=length`.
/// Returns `Some(n)` if we hit the cap, `None` if the model always stops early.
async fn probe_output_tokens(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
) -> Result<Option<u64>> {
    // Try progressively larger max_tokens until we get finish_reason=length
    for &max_tokens in &[200u64, 500, 1000, 2000, 4096, 8192] {
        let result = send_chat(client, base_url, api_key, model, OUTPUT_PROBE_PROMPT, max_tokens)
            .await?;

        if result.finish_reason.as_deref() == Some("length") {
            return Ok(Some(result.completion_tokens));
        }
    }

    // Model always stops before hitting the cap
    Ok(None)
}

// === HTTP helpers ===

struct ProbeUsage {
    prompt_tokens: u64,
}

struct ChatResult {
    finish_reason: Option<String>,
    prompt_tokens: u64,
    completion_tokens: u64,
}

async fn send_probe(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    approx_chars: usize,
) -> Result<ProbeUsage> {
    let content = PROBE_CONTENT.repeat(approx_chars / PROBE_CONTENT.len() + 1);
    let content = &content[..approx_chars];
    let prompt = format!("{content}\n\nSay ok");

    let result = send_chat(client, base_url, api_key, model, &prompt, 5).await?;
    Ok(ProbeUsage {
        prompt_tokens: result.prompt_tokens,
    })
}

async fn send_chat(
    client: &Client,
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    max_tokens: u64,
) -> Result<ChatResult> {
    let body = json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "stream": true,
        "stream_options": {"include_usage": true}
    });

    let resp = client
        .post(format!("{base_url}/v1/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("HTTP {status}: {text}"));
    }

    let text = resp.text().await?;
    let mut prompt_tokens: u64 = 0;
    let mut completion_tokens: u64 = 0;
    let mut finish_reason: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("data:") || line == "data: [DONE]" {
            continue;
        }
        let json_str = line.trim_start_matches("data:").trim();
        let Ok(chunk) = serde_json::from_str::<Value>(json_str) else {
            continue;
        };

        if let Some(usage) = chunk.get("usage") {
            prompt_tokens = usage["prompt_tokens"].as_u64().unwrap_or(0);
            completion_tokens = usage["completion_tokens"].as_u64().unwrap_or(0);
        }

        if let Some(choices) = chunk["choices"].as_array() {
            for choice in choices {
                if let Some(fr) = choice["finish_reason"].as_str() {
                    finish_reason = Some(fr.to_string());
                }
            }
        }
    }

    Ok(ChatResult {
        finish_reason,
        prompt_tokens,
        completion_tokens,
    })
}
