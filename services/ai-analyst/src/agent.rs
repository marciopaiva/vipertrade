//! Tuning agent: an LLM tool-use loop that runs the SAME analyses an operator
//! does by hand — read the trade diagnostics, form a hypothesis, run a
//! deterministic backtest sweep to test it, interpret the delta, and propose a
//! config change with quantified evidence.
//!
//! Provider-agnostic over the OpenAI-compatible `/v1/chat/completions` wire
//! format, so the SAME client drives a free local **Ollama** model (default)
//! or **Groq**'s hosted free tier — only the base URL, model and (optional) API
//! key change. The deterministic backtest is the source of truth; the model
//! only orchestrates tools and narrates, which keeps a small local model
//! viable. Endpoint: `POST /investigate`.

use crate::{run_sweep_core, ApiError, AppState, SweepCoreError, SweepRequest};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use warp::http::StatusCode;
use warp::{Rejection, Reply};

/// LLM endpoint config, resolved from the environment. `ollama` (default) needs
/// no key and points at the local server; `groq` uses the hosted free tier.
struct LlmConfig {
    base_url: String,
    model: String,
    api_key: Option<String>,
    max_iters: usize,
    temperature: f32,
}

impl LlmConfig {
    fn from_env() -> Self {
        let provider =
            std::env::var("AI_ANALYST_LLM_PROVIDER").unwrap_or_else(|_| "ollama".to_string());
        let (default_base, default_model, default_key) = match provider.as_str() {
            "groq" => (
                "https://api.groq.com/openai/v1",
                "llama-3.3-70b-versatile",
                std::env::var("GROQ_API_KEY").ok(),
            ),
            // Ollama's OpenAI-compatible endpoint; qwen2.5 supports tool calls.
            _ => ("http://localhost:11434/v1", "qwen2.5-coder:7b", None),
        };
        LlmConfig {
            base_url: std::env::var("AI_ANALYST_LLM_BASE_URL")
                .unwrap_or_else(|_| default_base.to_string()),
            model: std::env::var("AI_ANALYST_LLM_MODEL")
                .unwrap_or_else(|_| default_model.to_string()),
            api_key: std::env::var("AI_ANALYST_LLM_API_KEY")
                .ok()
                .or(default_key),
            max_iters: std::env::var("AI_ANALYST_LLM_MAX_ITERS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6),
            temperature: 0.2,
        }
    }

    fn provider_model(&self) -> String {
        format!("{} ({})", self.model, self.base_url)
    }
}

// ── OpenAI-compatible chat-completions wire types ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChatMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCall {
    #[serde(default)]
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FunctionCall {
    name: String,
    /// JSON-encoded arguments (a string, per the OpenAI wire format).
    #[serde(default)]
    arguments: String,
}

#[derive(Serialize)]
struct ToolDef {
    #[serde(rename = "type")]
    kind: &'static str,
    function: FunctionSchema,
}

#[derive(Serialize)]
struct FunctionSchema {
    name: &'static str,
    description: &'static str,
    parameters: Value,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    tools: &'a [ToolDef],
    tool_choice: &'a str,
    temperature: f32,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatRespMessage,
}

#[derive(Deserialize)]
struct ChatRespMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCall>>,
}

// ── Public endpoint ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct InvestigateRequest {
    /// What to investigate. Defaults to a "find the highest-impact lever" brief.
    question: Option<String>,
    /// Lookback for the diagnostics tool's default (hours).
    hours: Option<i64>,
}

#[derive(Serialize)]
struct InvestigateResponse {
    provider_model: String,
    iterations: usize,
    sweeps_run: usize,
    recommendation: Option<Value>,
    final_message: Option<String>,
    transcript: Vec<ChatMessage>,
}

const SYSTEM_PROMPT: &str = "\
You are ViperTrade's tuning analyst. Your job: find the single highest-impact \
config change to improve the strategy's net PnL, and justify it with a \
deterministic backtest — never guess.

Workflow:
1. Call get_trade_diagnostics to see the close_reason breakdown (which exit \
reason bleeds money, which is the edge).
2. Form ONE hypothesis (e.g. 'thesis_invalidated bleeds, loosen it').
3. Test it with run_backtest_sweep: a baseline plus one variant per change. The \
backtest replays the real recorded corpus through the real decision+exit logic, \
so the comparison is deterministic and trustworthy. Read each variant's \
delta_net_pnl.
4. Iterate: if the delta is small or negative, try a different lever or value.
5. When you have a change with a clear positive delta_net_pnl backed by a sweep, \
call propose_config_change with the exact overrides and the measured effect.

CRITICAL config-path rule: tunable params live under the mode profile. Use the \
full path 'mode_profiles.PAPER.<param>'. A bare path, or a param name that does \
not exist, is SILENTLY IGNORED — if a sweep returns delta_net_pnl exactly 0.0, \
your override did nothing: you used a wrong path or invented a param. Do NOT \
repeat the same failed override; pick a REAL param below or change the value.

The ONLY real tunable params (use these exact names, prefixed mode_profiles.PAPER.):
- min_adx (entry trend-strength gate, e.g. 18..30)
- stop_loss_pct (e.g. 0.012)
- max_percent_b_long / min_percent_b_short (Bollinger %B entry guard)
- rsi_long_min / rsi_long_max / rsi_short_min / rsi_short_max
- min_trend_score_long / min_trend_score_short
- thesis_health.long_invalidate / .long_invalidate_confirmed / .long_no_alignment \
/ .long_degrading_hard / .long_degrading_soft (negative ints; push past -100 to \
DISABLE that exit tier) and the short_* mirror (positive ints; push past +100).
To DISABLE thesis-invalidation exits entirely, set all thesis_health.long_* to \
-200 and all thesis_health.short_* to 200 in ONE variant.

Ground every claim in a sweep result. Be concise. Do not propose a change you \
have not backtested.

CRITICAL — act, don't narrate: when you decide to run a sweep, EMIT the \
run_backtest_sweep tool call immediately. NEVER end your turn with prose that \
merely says you will run a sweep ('Let's test...', 'Next I will...') — that \
wastes the turn and stalls. Only produce a plain-text answer when you call \
propose_config_change or are completely done.";

pub async fn handle_investigate(
    req: InvestigateRequest,
    state: Arc<AppState>,
) -> Result<impl Reply, Rejection> {
    let cfg = LlmConfig::from_env();
    let default_hours = req.hours.filter(|h| *h > 0).unwrap_or(168);
    let question = req.question.unwrap_or_else(|| {
        "Find the single highest-impact config change to improve net PnL. Use the \
         diagnostics, then prove it with a backtest sweep."
            .to_string()
    });

    let tools = tool_defs();
    let mut messages = vec![
        ChatMessage {
            role: "system".to_string(),
            content: Some(SYSTEM_PROMPT.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
        ChatMessage {
            role: "user".to_string(),
            content: Some(question),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        },
    ];

    let mut recommendation: Option<Value> = None;
    let mut final_message: Option<String> = None;
    let mut sweeps_run = 0usize;
    let mut iterations = 0usize;

    for _ in 0..cfg.max_iters {
        iterations += 1;
        let resp = match call_llm(&state, &cfg, &messages, &tools).await {
            Ok(r) => r,
            Err(err) => {
                error!("LLM call failed: {err}");
                return Ok(warp::reply::with_status(
                    warp::reply::json(&ApiError {
                        error: "llm_call_failed",
                        message: err,
                    }),
                    StatusCode::BAD_GATEWAY,
                ));
            }
        };

        let Some(choice) = resp.choices.into_iter().next() else {
            warn!("LLM returned no choices");
            break;
        };
        let msg = choice.message;

        // Prefer structured tool_calls; fall back to parsing a tool call emitted
        // as plain content (some free local models — e.g. qwen2.5-coder — don't
        // fill the structured field and write the call as text instead).
        let calls: Option<Vec<ToolCall>> = msg
            .tool_calls
            .clone()
            .filter(|c| !c.is_empty())
            .or_else(|| {
                msg.content
                    .as_deref()
                    .and_then(extract_tool_call_from_content)
                    .map(|tc| vec![tc])
            });

        // Record the assistant turn (with any tool_calls) into history.
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: msg.content.clone(),
            tool_calls: calls.clone(),
            tool_call_id: None,
            name: None,
        });

        match calls {
            Some(calls) if !calls.is_empty() => {
                for call in calls {
                    let result = dispatch_tool(
                        &state,
                        &call,
                        default_hours,
                        &mut recommendation,
                        &mut sweeps_run,
                    )
                    .await;
                    messages.push(ChatMessage {
                        role: "tool".to_string(),
                        content: Some(result),
                        tool_calls: None,
                        tool_call_id: Some(call.id.clone()),
                        name: Some(call.function.name.clone()),
                    });
                }
                // A config proposal is terminal.
                if recommendation.is_some() {
                    break;
                }
            }
            // No tool calls => the model gave a final answer.
            _ => {
                final_message = msg.content;
                break;
            }
        }
    }

    // Drop the system prompt from the returned transcript (keep it lean).
    let transcript: Vec<ChatMessage> = messages.into_iter().skip(1).collect();
    Ok(warp::reply::with_status(
        warp::reply::json(&InvestigateResponse {
            provider_model: cfg.provider_model(),
            iterations,
            sweeps_run,
            recommendation,
            final_message,
            transcript,
        }),
        StatusCode::OK,
    ))
}

// ── Tool definitions + dispatch ─────────────────────────────────────────────

fn tool_defs() -> Vec<ToolDef> {
    let override_item = json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "dotted config path, e.g. mode_profiles.PAPER.min_adx"},
            "value": {"type": "string", "description": "the value as a string, e.g. \"20\""}
        },
        "required": ["path", "value"]
    });
    vec![
        ToolDef {
            kind: "function",
            function: FunctionSchema {
                name: "get_trade_diagnostics",
                description: "Closed-trade breakdown by close_reason (count, net pnl, avg pnl, win%) over a lookback window. Use this first to see which exit reason bleeds and which is the edge.",
                parameters: json!({
                    "type": "object",
                    "properties": {"hours": {"type": "integer", "description": "lookback window in hours"}}
                }),
            },
        },
        ToolDef {
            kind: "function",
            function: FunctionSchema {
                name: "run_backtest_sweep",
                description: "Deterministically replay the recorded corpus through baseline plus one variant per config change, returning each variant's net-PnL delta vs baseline. This is the source of truth — test every hypothesis here.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "since": {"type": "string", "description": "optional RFC3339; restrict corpus to rows at/after this"},
                        "limit": {"type": "integer", "description": "max audit rows to replay (default 5000)"},
                        "variants": {
                            "type": "array",
                            "description": "one config variant per entry",
                            "items": {
                                "type": "object",
                                "properties": {"overrides": {"type": "array", "items": override_item.clone()}},
                                "required": ["overrides"]
                            }
                        }
                    },
                    "required": ["variants"]
                }),
            },
        },
        ToolDef {
            kind: "function",
            function: FunctionSchema {
                name: "propose_config_change",
                description: "Submit the final recommended config change, backed by a sweep. Call this only after a sweep showed a clear positive delta_net_pnl.",
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "rationale": {"type": "string"},
                        "expected_effect": {"type": "string", "description": "the measured delta, e.g. 'net -0.59 -> -0.03 (+0.56)'"},
                        "overrides": {"type": "array", "items": override_item}
                    },
                    "required": ["rationale", "overrides"]
                }),
            },
        },
    ]
}

/// Execute one tool call; returns the string fed back to the model as the tool
/// result. Errors are returned as messages (not failures) so the model can
/// self-correct on the next turn.
async fn dispatch_tool(
    state: &AppState,
    call: &ToolCall,
    default_hours: i64,
    recommendation: &mut Option<Value>,
    sweeps_run: &mut usize,
) -> String {
    let args: Value = serde_json::from_str(&call.function.arguments).unwrap_or(Value::Null);
    info!(tool = %call.function.name, "agent tool call");

    match call.function.name.as_str() {
        "get_trade_diagnostics" => {
            let hours = args.get("hours").and_then(Value::as_i64).unwrap_or(default_hours);
            match fetch_close_reason_summary(state, hours).await {
                Ok(rows) => json!({"hours": hours, "by_close_reason": rows}).to_string(),
                Err(e) => json!({"error": format!("diagnostics query failed: {e}")}).to_string(),
            }
        }
        "run_backtest_sweep" => {
            // Bound runtime: cap variants per call.
            let req: SweepRequest = match serde_json::from_value(args) {
                Ok(r) => r,
                Err(e) => {
                    return json!({
                        "error": format!("invalid arguments: {e}"),
                        "expected_schema": "{since?, limit?, variants:[{overrides:[{path,value}]}]}"
                    })
                    .to_string()
                }
            };
            *sweeps_run += 1;
            match run_sweep_core(state, &req).await {
                Ok(resp) => {
                    let mut v = serde_json::to_value(&resp).unwrap_or(Value::Null);
                    // Nudge weak models: if every variant moved net PnL by ~0, the
                    // overrides did nothing — flag it so they don't repeat it.
                    let all_zero = resp.result.variants.iter().all(|x| x.delta_net_pnl.abs() < 1e-9);
                    if !resp.result.variants.is_empty() && all_zero {
                        if let Value::Object(map) = &mut v {
                            map.insert("_hint".to_string(), json!(
                                "Every variant delta_net_pnl is 0.0 — your overrides changed nothing. \
                                 The param path or name is wrong. Use a REAL param from the system \
                                 prompt with the mode_profiles.PAPER. prefix; do not repeat this override."
                            ));
                        }
                    }
                    v.to_string()
                }
                Err(SweepCoreError::BadRequest(m)) | Err(SweepCoreError::Internal(m)) => {
                    json!({"error": m}).to_string()
                }
            }
        }
        "propose_config_change" => {
            *recommendation = Some(args.clone());
            json!({"status": "recorded"}).to_string()
        }
        other => json!({"error": format!("unknown tool: {other}")}).to_string(),
    }
}

/// Compact close_reason attribution — the table operators read first.
async fn fetch_close_reason_summary(
    state: &AppState,
    hours: i64,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows: Vec<(String, i64, f64, f64, f64)> = sqlx::query_as(
        "select coalesce(close_reason, 'unknown') as reason, \
                count(*)::int8 as n, \
                coalesce(sum(pnl), 0)::float8 as net, \
                coalesce(avg(pnl), 0)::float8 as avg_pnl, \
                (100.0 * sum((pnl > 0)::int) / greatest(count(*), 1))::float8 as win_pct \
         from trades \
         where status = 'closed' and closed_at >= now() - ($1 * interval '1 hour') \
         group by close_reason order by net asc",
    )
    .bind(hours)
    .fetch_all(&state.db_pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(reason, n, net, avg_pnl, win_pct)| {
            json!({
                "reason": reason,
                "trades": n,
                "net_pnl": net,
                "avg_pnl": avg_pnl,
                "win_pct": win_pct,
            })
        })
        .collect())
}

/// Recover a tool call a model wrote as plain content instead of structured
/// `tool_calls`. Only fires for a JSON object naming a KNOWN tool, so a normal
/// prose final answer is never mistaken for a call. Tolerates ```json fences
/// and surrounding prose by scanning for the outermost `{...}`.
fn extract_tool_call_from_content(content: &str) -> Option<ToolCall> {
    const KNOWN: [&str; 3] = [
        "get_trade_diagnostics",
        "run_backtest_sweep",
        "propose_config_change",
    ];
    let trimmed = content.trim();
    let unfenced = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .map(|s| s.strip_suffix("```").unwrap_or(s))
        .unwrap_or(trimmed)
        .trim();

    let mut candidates = vec![unfenced.to_string()];
    if let (Some(a), Some(b)) = (unfenced.find('{'), unfenced.rfind('}')) {
        if b > a {
            candidates.push(unfenced[a..=b].to_string());
        }
    }

    for cand in candidates {
        let Ok(v) = serde_json::from_str::<Value>(&cand) else {
            continue;
        };
        let Some(name) = v.get("name").and_then(Value::as_str) else {
            continue;
        };
        if !KNOWN.contains(&name) {
            continue;
        }
        let args = v
            .get("arguments")
            .or_else(|| v.get("parameters"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        // OpenAI `arguments` is a JSON-encoded string; the model may emit it as
        // either a nested object or an already-stringified blob.
        let arguments = match args {
            Value::String(s) => s,
            other => other.to_string(),
        };
        return Some(ToolCall {
            id: format!("call_{name}"),
            kind: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments,
            },
        });
    }
    None
}

/// POST one turn to the OpenAI-compatible endpoint and parse the response.
async fn call_llm(
    state: &AppState,
    cfg: &LlmConfig,
    messages: &[ChatMessage],
    tools: &[ToolDef],
) -> Result<ChatResponse, String> {
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let body = ChatRequest {
        model: &cfg.model,
        messages,
        tools,
        tool_choice: "auto",
        temperature: cfg.temperature,
        stream: false,
    };

    let mut builder = state
        .http_client
        .post(&url)
        .timeout(Duration::from_secs(240))
        .json(&body);
    if let Some(key) = &cfg.api_key {
        builder = builder.bearer_auth(key);
    }

    let resp = builder
        .send()
        .await
        .map_err(|e| format!("request to {url} failed: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("reading response body failed: {e}"))?;
    if !status.is_success() {
        return Err(format!("LLM returned {status}: {text}"));
    }
    serde_json::from_str::<ChatResponse>(&text)
        .map_err(|e| format!("parsing LLM response failed: {e} (body: {text})"))
}
