use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::convert::Infallible;
use std::env;
use std::fs;
use std::sync::Arc;
use tracing::{error, info};
use tupa_codegen::execution_plan::{codegen_pipeline, ExecutionPlan};
use tupa_parser::{parse_program, Item, PipelineDecl, Program};
use tupa_runtime::Runtime;
use tupa_typecheck::typecheck_program;
use warp::http::StatusCode;
use warp::{Filter, Rejection, Reply};

#[derive(Clone)]
struct AppState {
    db_pool: PgPool,
    http_client: Client,
    runtime: Runtime,
    execution_plan: Arc<ExecutionPlan>,
    llm_enabled: bool,
    ollama_url: Option<String>,
    ollama_model: Option<String>,
    default_lookback_hours: i64,
}

#[derive(Debug, Deserialize)]
struct AnalysisQuery {
    hours: Option<i64>,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    db_connected: bool,
    llm_enabled: bool,
}

#[derive(Debug, Serialize)]
struct MetricSummary {
    closed_trades: i64,
    total_pnl_usdt: f64,
    avg_pnl_pct: f64,
    avg_duration_s: f64,
    win_rate_pct: f64,
}

#[derive(Debug, Serialize)]
struct ExpectancyMetrics {
    winning_trades: i64,
    losing_trades: i64,
    neutral_trades: i64,
    avg_win_usdt: f64,
    avg_win_pct: f64,
    avg_loss_usdt: f64,
    avg_loss_pct: f64,
    payoff_ratio: f64,
    expectancy_usdt: f64,
    expectancy_pct: f64,
}

#[derive(Debug, Serialize)]
struct BreakdownItem {
    name: String,
    trades: i64,
    pnl_usdt: f64,
    avg_pnl_pct: f64,
    avg_duration_s: f64,
}

#[derive(Debug, Serialize)]
struct AnalystSnapshot {
    lookback_hours: i64,
    summary: SnapshotSummary,
    expectancy: ExpectancyMetrics,
    exits: SnapshotExitMetrics,
    sides: SnapshotSideMetrics,
    blockers: SnapshotBlockerMetrics,
    thesis: SnapshotThesisMetrics,
    symbols: SnapshotSymbolMetrics,
}

#[derive(Debug, Serialize)]
struct SnapshotSummary {
    closed_trades: i64,
    total_pnl_usdt: f64,
    avg_pnl_pct: f64,
    avg_duration_s: f64,
    win_rate_pct: f64,
}

#[derive(Debug, Serialize)]
struct SnapshotExitMetrics {
    thesis_invalidated_pct: f64,
    thesis_invalidated_avg_pnl_pct: f64,
    trailing_stop_pct: f64,
    trailing_stop_avg_pnl_pct: f64,
}

#[derive(Debug, Serialize)]
struct SnapshotSideMetrics {
    long_trade_share_pct: f64,
    short_trade_share_pct: f64,
    long_avg_pnl_pct: f64,
    short_avg_pnl_pct: f64,
}

#[derive(Debug, Serialize)]
struct SnapshotBlockerMetrics {
    top_reason: String,
    top_reason_hits: i64,
    consensus_blocks: i64,
    volume_blocks: i64,
    macd_blocks: i64,
}

#[derive(Debug, Serialize)]
struct SnapshotThesisMetrics {
    total_closes: i64,
    top_reason: String,
    top_reason_hits: i64,
    positive_close_pct: f64,
    long_avg_pnl_pct: f64,
    short_avg_pnl_pct: f64,
    no_alignment_hits: i64,
    health_threshold_hits: i64,
    opposite_side_hits: i64,
    consensus_trend_hits: i64,
    price_vs_fast_ema_hits: i64,
    btc_regime_hits: i64,
}

#[derive(Debug, Serialize)]
struct SnapshotSymbolMetrics {
    worst_symbol: String,
    worst_symbol_pnl_usdt: f64,
    best_symbol: String,
    best_symbol_pnl_usdt: f64,
}

#[derive(Debug, Serialize)]
struct AnalysisResponse {
    generated_at: DateTime<Utc>,
    lookback_hours: i64,
    summary: MetricSummary,
    expectancy: ExpectancyMetrics,
    by_close_reason: Vec<BreakdownItem>,
    by_side: Vec<BreakdownItem>,
    by_symbol: Vec<BreakdownItem>,
    top_entry_blockers: Vec<BlockerItem>,
    thesis_invalidation_breakdown: Vec<ThesisReasonItem>,
    comparative_diagnostics: ComparativeDiagnostics,
    recommendations: Vec<RecommendationItem>,
    symbol_diagnostics: Vec<SymbolDiagnosticItem>,
    tupa_snapshot: AnalystSnapshot,
    tupa_evaluation: Option<Value>,
    tupa_error: Option<String>,
    heuristic_summary: String,
    llm_summary: Option<String>,
}

#[derive(Debug, Serialize)]
struct BlockerItem {
    reason: String,
    total: i64,
}

#[derive(Debug, Serialize)]
struct ThesisReasonItem {
    reason: String,
    total: i64,
}

#[derive(Debug, Serialize)]
struct ComparativeMetric {
    current: f64,
    previous: f64,
    delta: f64,
}

#[derive(Debug, Serialize)]
struct ComparativeDiagnostics {
    status: String,
    reasons: Vec<String>,
    current_window_hours: i64,
    previous_window_hours: i64,
    closed_trades: ComparativeMetric,
    win_rate_pct: ComparativeMetric,
    expectancy_pct: ComparativeMetric,
    payoff_ratio: ComparativeMetric,
    thesis_invalidated_pct: ComparativeMetric,
    trailing_stop_pct: ComparativeMetric,
    long_avg_pnl_pct: ComparativeMetric,
    short_avg_pnl_pct: ComparativeMetric,
}

#[derive(Debug, Serialize)]
struct RecommendationItem {
    recommendation_id: String,
    severity: String,
    confidence: String,
    recommendation: String,
    evidence: String,
    expected_tradeoff: String,
}

#[derive(Debug, Serialize)]
struct SymbolDiagnosticItem {
    symbol: String,
    status: String,
    recommendation: String,
    confidence: String,
    trades: i64,
    avg_pnl_pct: f64,
    thesis_invalidated_trades: i64,
    trailing_stop_trades: i64,
    avg_thesis_pnl_pct: f64,
    avg_trailing_pnl_pct: f64,
}

#[derive(Debug)]
struct ThesisSummary {
    total_closes: i64,
    positive_closes: i64,
    long_avg_pnl_pct: f64,
    short_avg_pnl_pct: f64,
}

struct HeuristicSummaryContext<'a> {
    hours: i64,
    summary: &'a MetricSummary,
    expectancy: &'a ExpectancyMetrics,
    by_close_reason: &'a [BreakdownItem],
    by_side: &'a [BreakdownItem],
    by_symbol: &'a [BreakdownItem],
    blockers: &'a [BlockerItem],
    thesis_breakdown: &'a [ThesisReasonItem],
}

struct TupaSnapshotContext<'a> {
    hours: i64,
    summary: &'a MetricSummary,
    expectancy: &'a ExpectancyMetrics,
    by_close_reason: &'a [BreakdownItem],
    by_side: &'a [BreakdownItem],
    by_symbol: &'a [BreakdownItem],
    blockers: &'a [BlockerItem],
    thesis_summary: &'a ThesisSummary,
    thesis_breakdown: &'a [ThesisReasonItem],
}

#[derive(Debug, sqlx::FromRow)]
struct SymbolDiagnosticRow {
    symbol: String,
    trades: i64,
    avg_pnl_pct: f64,
    thesis_invalidated_trades: i64,
    trailing_stop_trades: i64,
    avg_thesis_pnl_pct: f64,
    avg_trailing_pnl_pct: f64,
}

#[derive(Debug, Serialize)]
struct ApiError {
    error: &'static str,
    message: String,
}

#[derive(thiserror::Error, Debug)]
enum AnalystError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("runtime error: {0}")]
    Runtime(String),
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "viper_ai_analyst=info".into()),
        )
        .json()
        .init();

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        let host = env::var("DB_HOST").unwrap_or_else(|_| "postgres".to_string());
        let port = env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
        let name = env::var("DB_NAME").unwrap_or_else(|_| "vipertrade".to_string());
        let user = env::var("DB_USER").unwrap_or_else(|_| "viper".to_string());
        let password = env::var("DB_PASSWORD").unwrap_or_default();
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            user, password, host, port, name
        )
    });

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    let pipeline_path = env::var("AI_ANALYST_TUPA_PIPELINE")
        .unwrap_or_else(|_| "/app/config/analysts/trade_diagnostics.tp".to_string());
    let execution_plan = Arc::new(load_execution_plan(&pipeline_path)?);
    let runtime = Runtime::new();
    register_analyst_steps(&runtime, execution_plan.as_ref());

    let llm_enabled = env::var("AI_ANALYST_ENABLE_LLM")
        .map(|value| value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let state = Arc::new(AppState {
        db_pool,
        http_client: Client::new(),
        runtime,
        execution_plan,
        llm_enabled,
        ollama_url: env::var("OLLAMA_URL").ok(),
        ollama_model: env::var("OLLAMA_MODEL").ok(),
        default_lookback_hours: env::var("AI_ANALYST_LOOKBACK_HOURS")
            .ok()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(24),
    });

    let health = warp::path!("health")
        .and(warp::get())
        .and(with_state(state.clone()))
        .and_then(handle_health);

    let analyze_recent = warp::path!("analyze" / "recent")
        .and(warp::get())
        .and(warp::query::<AnalysisQuery>())
        .and(with_state(state.clone()))
        .and_then(handle_recent_analysis);

    let routes = health
        .or(analyze_recent)
        .recover(handle_rejection)
        .with(warp::cors().allow_any_origin());

    info!("Starting viper-ai-analyst on :8087");
    warp::serve(routes).run(([0, 0, 0, 0], 8087)).await;
    Ok(())
}

fn with_state(
    state: Arc<AppState>,
) -> impl Filter<Extract = (Arc<AppState>,), Error = Infallible> + Clone {
    warp::any().map(move || state.clone())
}

async fn handle_health(state: Arc<AppState>) -> Result<impl Reply, Rejection> {
    let db_connected = sqlx::query_scalar::<_, i64>("select 1::bigint")
        .fetch_one(&state.db_pool)
        .await
        .is_ok();

    Ok(warp::reply::json(&HealthResponse {
        status: "ok",
        db_connected,
        llm_enabled: state.llm_enabled,
    }))
}

async fn handle_recent_analysis(
    query: AnalysisQuery,
    state: Arc<AppState>,
) -> Result<impl Reply, Rejection> {
    let hours = query
        .hours
        .filter(|value| *value > 0 && *value <= 24 * 14)
        .unwrap_or(state.default_lookback_hours);

    match build_analysis(hours, &state).await {
        Ok(response) => Ok(warp::reply::with_status(
            warp::reply::json(&response),
            StatusCode::OK,
        )),
        Err(err) => {
            error!("analysis failed: {err}");
            Ok(warp::reply::with_status(
                warp::reply::json(&ApiError {
                    error: "analysis_failed",
                    message: err.to_string(),
                }),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

async fn build_analysis(hours: i64, state: &AppState) -> Result<AnalysisResponse, AnalystError> {
    let summary = fetch_summary(hours, 0, &state.db_pool).await?;
    let expectancy = fetch_expectancy(hours, 0, &state.db_pool).await?;
    let by_close_reason = fetch_breakdown(
        hours,
        0,
        &state.db_pool,
        "close_reason",
        "select coalesce(close_reason, 'unknown') as name, count(*) as trades, coalesce(sum(pnl), 0)::float8 as pnl_usdt, coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct, coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s from trades where status='closed' and opened_at >= now() - (($1 + $2) * interval '1 hour') and opened_at < now() - ($2 * interval '1 hour') group by close_reason order by trades desc",
    )
    .await?;
    let by_side = fetch_breakdown(
        hours,
        0,
        &state.db_pool,
        "side",
        "select side as name, count(*) as trades, coalesce(sum(pnl), 0)::float8 as pnl_usdt, coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct, coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s from trades where status='closed' and opened_at >= now() - (($1 + $2) * interval '1 hour') and opened_at < now() - ($2 * interval '1 hour') group by side order by trades desc",
    )
    .await?;
    let by_symbol = fetch_breakdown(
        hours,
        0,
        &state.db_pool,
        "symbol",
        "select symbol as name, count(*) as trades, coalesce(sum(pnl), 0)::float8 as pnl_usdt, coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct, coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s from trades where status='closed' and opened_at >= now() - (($1 + $2) * interval '1 hour') and opened_at < now() - ($2 * interval '1 hour') group by symbol order by pnl_usdt asc limit 10",
    )
    .await?;
    let top_entry_blockers = fetch_top_blockers(hours, 0, &state.db_pool).await?;
    let thesis_summary = fetch_thesis_summary(hours, 0, &state.db_pool).await?;
    let thesis_invalidation_breakdown =
        fetch_thesis_invalidation_breakdown(hours, 0, &state.db_pool).await?;
    let symbol_diagnostics = fetch_symbol_diagnostics(hours, 0, &state.db_pool).await?;

    let previous_summary = fetch_summary(hours, hours, &state.db_pool).await?;
    let previous_expectancy = fetch_expectancy(hours, hours, &state.db_pool).await?;
    let previous_by_close_reason = fetch_breakdown(
        hours,
        hours,
        &state.db_pool,
        "close_reason",
        "select coalesce(close_reason, 'unknown') as name, count(*) as trades, coalesce(sum(pnl), 0)::float8 as pnl_usdt, coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct, coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s from trades where status='closed' and opened_at >= now() - (($1 + $2) * interval '1 hour') and opened_at < now() - ($2 * interval '1 hour') group by close_reason order by trades desc",
    )
    .await?;
    let previous_by_side = fetch_breakdown(
        hours,
        hours,
        &state.db_pool,
        "side",
        "select side as name, count(*) as trades, coalesce(sum(pnl), 0)::float8 as pnl_usdt, coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct, coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s from trades where status='closed' and opened_at >= now() - (($1 + $2) * interval '1 hour') and opened_at < now() - ($2 * interval '1 hour') group by side order by trades desc",
    )
    .await?;
    let tupa_snapshot = build_tupa_snapshot(TupaSnapshotContext {
        hours,
        summary: &summary,
        expectancy: &expectancy,
        by_close_reason: &by_close_reason,
        by_side: &by_side,
        by_symbol: &by_symbol,
        blockers: &top_entry_blockers,
        thesis_summary: &thesis_summary,
        thesis_breakdown: &thesis_invalidation_breakdown,
    });
    let comparative_diagnostics =
        build_comparative_diagnostics(ComparativeDiagnosticsContext {
            hours,
            current_summary: &summary,
            previous_summary: &previous_summary,
            current_expectancy: &expectancy,
            previous_expectancy: &previous_expectancy,
            current_by_close_reason: &by_close_reason,
            previous_by_close_reason: &previous_by_close_reason,
            current_by_side: &by_side,
            previous_by_side: &previous_by_side,
        });
    let (tupa_evaluation, tupa_error) = match run_tupa_diagnostics(&tupa_snapshot, state).await {
        Ok(value) => (Some(value), None),
        Err(err) => {
            error!("tupa diagnostics failed: {err}");
            (None, Some(err.to_string()))
        }
    };
    let recommendations = build_recommendations(
        &summary,
        &expectancy,
        &comparative_diagnostics,
        &top_entry_blockers,
        &symbol_diagnostics,
        tupa_evaluation.as_ref(),
    );

    let heuristic_summary = build_heuristic_summary(HeuristicSummaryContext {
        hours,
        summary: &summary,
        expectancy: &expectancy,
        by_close_reason: &by_close_reason,
        by_side: &by_side,
        by_symbol: &by_symbol,
        blockers: &top_entry_blockers,
        thesis_breakdown: &thesis_invalidation_breakdown,
    });
    let llm_summary = if state.llm_enabled {
        request_llm_summary(state, &heuristic_summary).await?
    } else {
        None
    };

    Ok(AnalysisResponse {
        generated_at: Utc::now(),
        lookback_hours: hours,
        summary,
        expectancy,
        by_close_reason,
        by_side,
        by_symbol,
        top_entry_blockers,
        thesis_invalidation_breakdown,
        comparative_diagnostics,
        recommendations,
        symbol_diagnostics,
        tupa_snapshot,
        tupa_evaluation,
        tupa_error,
        heuristic_summary,
        llm_summary,
    })
}

fn first_pipeline(program: &Program) -> Result<&PipelineDecl, AnalystError> {
    program
        .items
        .iter()
        .find_map(|item| match item {
            Item::Pipeline(p) => Some(p),
            _ => None,
        })
        .ok_or_else(|| AnalystError::Runtime("no pipeline declaration found".to_string()))
}

fn load_execution_plan(path: &str) -> Result<ExecutionPlan, AnalystError> {
    let source = fs::read_to_string(path)?;
    let program = parse_program(&source).map_err(|err| AnalystError::Runtime(err.to_string()))?;

    if let Err(err) = typecheck_program(&program) {
        info!("Trade diagnostics typecheck warning (continuing): {}", err);
    }

    let pipeline = first_pipeline(&program)?;
    let plan_json = codegen_pipeline("ai_analyst", pipeline, &program)
        .map_err(|err| AnalystError::Runtime(err.to_string()))?;
    let plan: ExecutionPlan = serde_json::from_str(&plan_json)?;
    Ok(plan)
}

fn register_analyst_steps(runtime: &Runtime, plan: &ExecutionPlan) {
    for step in &plan.steps {
        let function_ref = step.function_ref.clone();
        let fallback_name = step.name.clone();
        let fn_name = function_ref
            .rsplit("::")
            .next()
            .unwrap_or(&fallback_name)
            .to_string();

        runtime.register_step(&function_ref, move |state| match fn_name.as_str() {
            "evaluate_exit_pressure" | "step_exit_pressure" => execute_exit_pressure(state),
            "evaluate_directional_bias" | "step_directional_bias" => {
                execute_directional_bias(state)
            }
            "evaluate_entry_pressure" | "step_entry_pressure" => execute_entry_pressure(state),
            "evaluate_thesis_quality" | "step_thesis_quality" => execute_thesis_quality(state),
            "evaluate_symbol_risk" | "step_symbol_risk" => execute_symbol_risk(state),
            other => Err(format!("unknown analyst step {}", other)),
        });
    }
}

async fn run_tupa_diagnostics(
    snapshot: &AnalystSnapshot,
    state: &AppState,
) -> Result<Value, AnalystError> {
    let input = serde_json::to_value(snapshot)?;
    state
        .runtime
        .run_pipeline_async(state.execution_plan.as_ref(), input)
        .await
        .map_err(|err| AnalystError::Runtime(err.to_string()))
}

fn as_f64(value: &Value, path: &[&str]) -> Result<f64, String> {
    let mut current = value;
    for part in path {
        current = current
            .get(*part)
            .ok_or_else(|| format!("missing field {}", path.join(".")))?;
    }
    current
        .as_f64()
        .ok_or_else(|| format!("expected f64 at {}", path.join(".")))
}

fn as_i64(value: &Value, path: &[&str]) -> Result<i64, String> {
    let mut current = value;
    for part in path {
        current = current
            .get(*part)
            .ok_or_else(|| format!("missing field {}", path.join(".")))?;
    }
    current
        .as_i64()
        .ok_or_else(|| format!("expected i64 at {}", path.join(".")))
}

fn as_str_value(value: &Value, path: &[&str]) -> Result<String, String> {
    let mut current = value;
    for part in path {
        current = current
            .get(*part)
            .ok_or_else(|| format!("missing field {}", path.join(".")))?;
    }
    current
        .as_str()
        .map(|value| value.to_string())
        .ok_or_else(|| format!("expected string at {}", path.join(".")))
}

fn execute_exit_pressure(state: Value) -> Result<Value, String> {
    let thesis_invalidated_pct = as_f64(&state, &["exits", "thesis_invalidated_pct"])?;
    let trailing_stop_pct = as_f64(&state, &["exits", "trailing_stop_pct"])?;

    let (severity, reason) = if thesis_invalidated_pct >= 80.0 && trailing_stop_pct <= 12.0 {
        ("fail", "exit_pressure_high")
    } else if thesis_invalidated_pct >= 65.0 {
        ("warn", "exit_pressure_elevated")
    } else {
        ("pass", "exit_pressure_stable")
    };

    Ok(json!({
        "severity": severity,
        "reason": reason,
        "thesis_invalidated_pct": thesis_invalidated_pct,
        "trailing_stop_pct": trailing_stop_pct
    }))
}

fn execute_directional_bias(state: Value) -> Result<Value, String> {
    let long_avg_pnl_pct = as_f64(&state, &["sides", "long_avg_pnl_pct"])?;
    let short_avg_pnl_pct = as_f64(&state, &["sides", "short_avg_pnl_pct"])?;

    let (score, reason) = if long_avg_pnl_pct >= short_avg_pnl_pct {
        (1.0, "directional_bias_long")
    } else {
        (0.0, "directional_bias_short")
    };

    Ok(json!({
        "score": score,
        "weight": 100.0,
        "reason": reason
    }))
}

fn execute_entry_pressure(state: Value) -> Result<Value, String> {
    let consensus_blocks = as_i64(&state, &["blockers", "consensus_blocks"])?;
    let volume_blocks = as_i64(&state, &["blockers", "volume_blocks"])?;
    let macd_blocks = as_i64(&state, &["blockers", "macd_blocks"])?;

    let (reason, dominant_gate) =
        if consensus_blocks >= volume_blocks && consensus_blocks >= macd_blocks {
            ("entry_pressure_consensus", "consensus")
        } else if volume_blocks >= macd_blocks {
            ("entry_pressure_volume", "volume")
        } else {
            ("entry_pressure_macd", "macd")
        };

    Ok(json!({
        "severity": "warn",
        "reason": reason,
        "dominant_gate": dominant_gate
    }))
}

fn execute_symbol_risk(state: Value) -> Result<Value, String> {
    let worst_symbol = as_str_value(&state, &["symbols", "worst_symbol"])?;
    let worst_symbol_pnl_usdt = as_f64(&state, &["symbols", "worst_symbol_pnl_usdt"])?;

    let (severity, reason) = if worst_symbol_pnl_usdt <= -0.30 {
        ("fail", "symbol_risk_high")
    } else if worst_symbol_pnl_usdt < 0.0 {
        ("warn", "symbol_risk_elevated")
    } else {
        ("pass", "symbol_risk_stable")
    };

    Ok(json!({
        "severity": severity,
        "reason": reason,
        "symbol": worst_symbol
    }))
}

fn execute_thesis_quality(state: Value) -> Result<Value, String> {
    let positive_close_pct = as_f64(&state, &["thesis", "positive_close_pct"])?;
    let long_avg_pnl_pct = as_f64(&state, &["thesis", "long_avg_pnl_pct"])?;
    let short_avg_pnl_pct = as_f64(&state, &["thesis", "short_avg_pnl_pct"])?;
    let top_reason = as_str_value(&state, &["thesis", "top_reason"])?;
    let no_alignment_hits = as_i64(&state, &["thesis", "no_alignment_hits"])?;
    let health_threshold_hits = as_i64(&state, &["thesis", "health_threshold_hits"])?;

    let (severity, reason, recommendation) = if long_avg_pnl_pct <= -0.20
        && no_alignment_hits >= health_threshold_hits
    {
        (
            "fail",
            "thesis_quality_long_fragile",
            "harden_long_invalidation_inputs",
        )
    } else if positive_close_pct >= 25.0 {
        (
            "pass",
            "thesis_quality_profit_protective",
            "preserve_trailing_capture",
        )
    } else if top_reason.contains("health_threshold") || health_threshold_hits > no_alignment_hits {
        (
            "warn",
            "thesis_quality_threshold_driven",
            "review_health_threshold_balance",
        )
    } else if short_avg_pnl_pct >= long_avg_pnl_pct {
        (
            "warn",
            "thesis_quality_directionally_asymmetric",
            "review_long_side_guard",
        )
    } else {
        (
            "pass",
            "thesis_quality_stable",
            "keep_current_thesis_policy",
        )
    };

    Ok(json!({
        "severity": severity,
        "reason": reason,
        "recommendation": recommendation,
        "positive_close_pct": positive_close_pct,
        "long_avg_pnl_pct": long_avg_pnl_pct,
        "short_avg_pnl_pct": short_avg_pnl_pct,
        "top_reason": top_reason
    }))
}

async fn fetch_summary(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<MetricSummary, AnalystError> {
    let row = sqlx::query_as::<_, (i64, f64, f64, f64, f64)>(
        r#"
        select
          count(*)::bigint as closed_trades,
          coalesce(sum(pnl), 0)::float8 as total_pnl_usdt,
          coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct,
          coalesce(avg(duration_seconds), 0)::float8 as avg_duration_s,
          coalesce(100.0 * count(*) filter (where pnl > 0) / nullif(count(*), 0), 0)::float8 as win_rate_pct
        from trades
        where status = 'closed'
          and opened_at >= now() - (($1 + $2) * interval '1 hour')
          and opened_at < now() - ($2 * interval '1 hour')
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_one(pool)
    .await?;

    Ok(MetricSummary {
        closed_trades: row.0,
        total_pnl_usdt: row.1,
        avg_pnl_pct: row.2,
        avg_duration_s: row.3,
        win_rate_pct: row.4,
    })
}

async fn fetch_expectancy(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<ExpectancyMetrics, AnalystError> {
    let row = sqlx::query_as::<_, (i64, i64, i64, f64, f64, f64, f64)>(
        r#"
        select
          count(*) filter (where pnl > 0)::bigint as winning_trades,
          count(*) filter (where pnl < 0)::bigint as losing_trades,
          count(*) filter (where pnl = 0)::bigint as neutral_trades,
          coalesce(avg(pnl) filter (where pnl > 0), 0)::float8 as avg_win_usdt,
          coalesce(avg(pnl_pct) filter (where pnl > 0), 0)::float8 as avg_win_pct,
          coalesce(avg(pnl) filter (where pnl < 0), 0)::float8 as avg_loss_usdt,
          coalesce(avg(pnl_pct) filter (where pnl < 0), 0)::float8 as avg_loss_pct
        from trades
        where status = 'closed'
          and opened_at >= now() - (($1 + $2) * interval '1 hour')
          and opened_at < now() - ($2 * interval '1 hour')
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_one(pool)
    .await?;

    let total_trades = (row.0 + row.1 + row.2).max(1) as f64;
    let payoff_ratio = if row.5.abs() > f64::EPSILON {
        (row.3 / row.5.abs()).max(0.0)
    } else {
        0.0
    };
    let expectancy_usdt = ((row.0 as f64 * row.3) + (row.1 as f64 * row.5)) / total_trades;
    let expectancy_pct = ((row.0 as f64 * row.4) + (row.1 as f64 * row.6)) / total_trades;

    Ok(ExpectancyMetrics {
        winning_trades: row.0,
        losing_trades: row.1,
        neutral_trades: row.2,
        avg_win_usdt: row.3,
        avg_win_pct: row.4,
        avg_loss_usdt: row.5,
        avg_loss_pct: row.6,
        payoff_ratio,
        expectancy_usdt,
        expectancy_pct,
    })
}

async fn fetch_breakdown(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
    _dimension: &str,
    sql: &str,
) -> Result<Vec<BreakdownItem>, AnalystError> {
    let rows = sqlx::query_as::<_, (String, i64, f64, f64, f64)>(sql)
        .bind(hours)
        .bind(offset_hours)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(
            |(name, trades, pnl_usdt, avg_pnl_pct, avg_duration_s)| BreakdownItem {
                name,
                trades,
                pnl_usdt,
                avg_pnl_pct,
                avg_duration_s,
            },
        )
        .collect())
}

async fn fetch_top_blockers(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<Vec<BlockerItem>, AnalystError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        select
          constraints_results->'validate_entry'->>'reason' as reason,
          count(*)::bigint as total
        from tupa_audit_logs
        where created_at >= now() - (($1 + $2) * interval '1 hour')
          and created_at < now() - ($2 * interval '1 hour')
          and (constraints_results->'validate_entry'->>'passed')::boolean = false
        group by reason
        order by total desc
        limit 8
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(reason, total)| BlockerItem { reason, total })
        .collect())
}

async fn fetch_thesis_summary(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<ThesisSummary, AnalystError> {
    let row = sqlx::query_as::<_, (i64, i64, f64, f64)>(
        r#"
        select
          count(*)::bigint as total_closes,
          count(*) filter (where pnl > 0)::bigint as positive_closes,
          coalesce(avg(pnl_pct) filter (where side = 'Long'), 0)::float8 as long_avg_pnl_pct,
          coalesce(avg(pnl_pct) filter (where side = 'Short'), 0)::float8 as short_avg_pnl_pct
        from trades
        where status = 'closed'
          and close_reason = 'thesis_invalidated'
          and opened_at >= now() - (($1 + $2) * interval '1 hour')
          and opened_at < now() - ($2 * interval '1 hour')
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_one(pool)
    .await?;

    Ok(ThesisSummary {
        total_closes: row.0,
        positive_closes: row.1,
        long_avg_pnl_pct: row.2,
        short_avg_pnl_pct: row.3,
    })
}

async fn fetch_thesis_invalidation_breakdown(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<Vec<ThesisReasonItem>, AnalystError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        select
          reason,
          count(*)::bigint as total
        from strategy_decision_audit
        where created_at >= now() - (($1 + $2) * interval '1 hour')
          and created_at < now() - ($2 * interval '1 hour')
          and action in ('CLOSE_LONG', 'CLOSE_SHORT')
          and reason like 'thesis_invalidated%'
        group by reason
        order by total desc
        limit 12
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(reason, total)| ThesisReasonItem { reason, total })
        .collect())
}

async fn fetch_symbol_diagnostics(
    hours: i64,
    offset_hours: i64,
    pool: &PgPool,
) -> Result<Vec<SymbolDiagnosticItem>, AnalystError> {
    let rows = sqlx::query_as::<_, SymbolDiagnosticRow>(
        r#"
        select
          symbol,
          count(*)::bigint as trades,
          coalesce(avg(pnl_pct), 0)::float8 as avg_pnl_pct,
          count(*) filter (where close_reason = 'thesis_invalidated')::bigint as thesis_invalidated_trades,
          count(*) filter (where close_reason = 'trailing_stop')::bigint as trailing_stop_trades,
          coalesce(avg(pnl_pct) filter (where close_reason = 'thesis_invalidated'), 0)::float8 as avg_thesis_pnl_pct,
          coalesce(avg(pnl_pct) filter (where close_reason = 'trailing_stop'), 0)::float8 as avg_trailing_pnl_pct
        from trades
        where status = 'closed'
          and opened_at >= now() - (($1 + $2) * interval '1 hour')
          and opened_at < now() - ($2 * interval '1 hour')
        group by symbol
        order by avg_pnl_pct asc, trades desc
        limit 12
        "#,
    )
    .bind(hours)
    .bind(offset_hours)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let (status, recommendation, confidence) = if row.trades >= 2
                && row.avg_pnl_pct < 0.0
                && row.thesis_invalidated_trades >= row.trailing_stop_trades
            {
                (
                    "fragile".to_string(),
                    "tighten_entry_or_reduce_priority".to_string(),
                    "high".to_string(),
                )
            } else if row.avg_pnl_pct > 0.0
                && row.trailing_stop_trades > row.thesis_invalidated_trades
            {
                (
                    "healthy".to_string(),
                    "keep_current_policy".to_string(),
                    "medium".to_string(),
                )
            } else {
                (
                    "mixed".to_string(),
                    "observe_more_sample".to_string(),
                    "medium".to_string(),
                )
            };

            SymbolDiagnosticItem {
                symbol: row.symbol,
                status,
                recommendation,
                confidence,
                trades: row.trades,
                avg_pnl_pct: row.avg_pnl_pct,
                thesis_invalidated_trades: row.thesis_invalidated_trades,
                trailing_stop_trades: row.trailing_stop_trades,
                avg_thesis_pnl_pct: row.avg_thesis_pnl_pct,
                avg_trailing_pnl_pct: row.avg_trailing_pnl_pct,
            }
        })
        .collect())
}

fn comparative_metric(current: f64, previous: f64) -> ComparativeMetric {
    ComparativeMetric {
        current,
        previous,
        delta: current - previous,
    }
}

fn breakdown_metric<F>(items: &[BreakdownItem], name: &str, field: F) -> f64
where
    F: Fn(&BreakdownItem) -> f64,
{
    items
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case(name))
        .map(field)
        .unwrap_or(0.0)
}

struct ComparativeDiagnosticsContext<'a> {
    hours: i64,
    current_summary: &'a MetricSummary,
    previous_summary: &'a MetricSummary,
    current_expectancy: &'a ExpectancyMetrics,
    previous_expectancy: &'a ExpectancyMetrics,
    current_by_close_reason: &'a [BreakdownItem],
    previous_by_close_reason: &'a [BreakdownItem],
    current_by_side: &'a [BreakdownItem],
    previous_by_side: &'a [BreakdownItem],
}

fn build_comparative_diagnostics(
    ctx: ComparativeDiagnosticsContext<'_>,
) -> ComparativeDiagnostics {
    let ComparativeDiagnosticsContext {
        hours,
        current_summary,
        previous_summary,
        current_expectancy,
        previous_expectancy,
        current_by_close_reason,
        previous_by_close_reason,
        current_by_side,
        previous_by_side,
    } = ctx;
    let thesis_invalidated_pct_current =
        breakdown_metric(current_by_close_reason, "thesis_invalidated", |item| {
            if current_summary.closed_trades > 0 {
                100.0 * item.trades as f64 / current_summary.closed_trades as f64
            } else {
                0.0
            }
        });
    let thesis_invalidated_pct_previous =
        breakdown_metric(previous_by_close_reason, "thesis_invalidated", |item| {
            if previous_summary.closed_trades > 0 {
                100.0 * item.trades as f64 / previous_summary.closed_trades as f64
            } else {
                0.0
            }
        });
    let trailing_stop_pct_current =
        breakdown_metric(current_by_close_reason, "trailing_stop", |item| {
            if current_summary.closed_trades > 0 {
                100.0 * item.trades as f64 / current_summary.closed_trades as f64
            } else {
                0.0
            }
        });
    let trailing_stop_pct_previous =
        breakdown_metric(previous_by_close_reason, "trailing_stop", |item| {
            if previous_summary.closed_trades > 0 {
                100.0 * item.trades as f64 / previous_summary.closed_trades as f64
            } else {
                0.0
            }
        });
    let long_avg_current = breakdown_metric(current_by_side, "long", |item| item.avg_pnl_pct);
    let long_avg_previous = breakdown_metric(previous_by_side, "long", |item| item.avg_pnl_pct);
    let short_avg_current = breakdown_metric(current_by_side, "short", |item| item.avg_pnl_pct);
    let short_avg_previous = breakdown_metric(previous_by_side, "short", |item| item.avg_pnl_pct);

    if previous_summary.closed_trades == 0 {
        return ComparativeDiagnostics {
            status: "insufficient_baseline".to_string(),
            reasons: vec!["previous_window_empty".to_string()],
            current_window_hours: hours,
            previous_window_hours: hours,
            closed_trades: comparative_metric(
                current_summary.closed_trades as f64,
                previous_summary.closed_trades as f64,
            ),
            win_rate_pct: comparative_metric(
                current_summary.win_rate_pct,
                previous_summary.win_rate_pct,
            ),
            expectancy_pct: comparative_metric(
                current_expectancy.expectancy_pct,
                previous_expectancy.expectancy_pct,
            ),
            payoff_ratio: comparative_metric(
                current_expectancy.payoff_ratio,
                previous_expectancy.payoff_ratio,
            ),
            thesis_invalidated_pct: comparative_metric(
                thesis_invalidated_pct_current,
                thesis_invalidated_pct_previous,
            ),
            trailing_stop_pct: comparative_metric(
                trailing_stop_pct_current,
                trailing_stop_pct_previous,
            ),
            long_avg_pnl_pct: comparative_metric(long_avg_current, long_avg_previous),
            short_avg_pnl_pct: comparative_metric(short_avg_current, short_avg_previous),
        };
    }

    let mut reasons = Vec::new();
    if current_expectancy.expectancy_pct > previous_expectancy.expectancy_pct + 0.10 {
        reasons.push("improved_expectancy".to_string());
    } else if current_expectancy.expectancy_pct + 0.10 < previous_expectancy.expectancy_pct {
        reasons.push("regressed_expectancy".to_string());
    }

    if thesis_invalidated_pct_current + 5.0 < thesis_invalidated_pct_previous {
        reasons.push("improved_exit_mix".to_string());
    } else if thesis_invalidated_pct_current > thesis_invalidated_pct_previous + 5.0 {
        reasons.push("regressed_exit_mix".to_string());
    }

    if long_avg_current + 0.10 < long_avg_previous {
        reasons.push("regressed_long_side".to_string());
    } else if long_avg_current > long_avg_previous + 0.10 {
        reasons.push("improved_long_side".to_string());
    }

    let status = if reasons.iter().any(|reason| reason.starts_with("regressed"))
        && reasons.iter().any(|reason| reason.starts_with("improved"))
    {
        "mixed"
    } else if reasons.iter().any(|reason| reason.starts_with("regressed")) {
        "regressed"
    } else if reasons.iter().any(|reason| reason.starts_with("improved")) {
        "improved"
    } else {
        "stable"
    };

    ComparativeDiagnostics {
        status: status.to_string(),
        reasons,
        current_window_hours: hours,
        previous_window_hours: hours,
        closed_trades: comparative_metric(
            current_summary.closed_trades as f64,
            previous_summary.closed_trades as f64,
        ),
        win_rate_pct: comparative_metric(
            current_summary.win_rate_pct,
            previous_summary.win_rate_pct,
        ),
        expectancy_pct: comparative_metric(
            current_expectancy.expectancy_pct,
            previous_expectancy.expectancy_pct,
        ),
        payoff_ratio: comparative_metric(
            current_expectancy.payoff_ratio,
            previous_expectancy.payoff_ratio,
        ),
        thesis_invalidated_pct: comparative_metric(
            thesis_invalidated_pct_current,
            thesis_invalidated_pct_previous,
        ),
        trailing_stop_pct: comparative_metric(
            trailing_stop_pct_current,
            trailing_stop_pct_previous,
        ),
        long_avg_pnl_pct: comparative_metric(long_avg_current, long_avg_previous),
        short_avg_pnl_pct: comparative_metric(short_avg_current, short_avg_previous),
    }
}

fn build_recommendations(
    summary: &MetricSummary,
    expectancy: &ExpectancyMetrics,
    comparative: &ComparativeDiagnostics,
    blockers: &[BlockerItem],
    symbols: &[SymbolDiagnosticItem],
    tupa_evaluation: Option<&Value>,
) -> Vec<RecommendationItem> {
    let mut items = Vec::new();

    if let Some(thesis) = tupa_evaluation.and_then(|value| value.get("thesis_quality")) {
        let severity = thesis
            .get("severity")
            .and_then(Value::as_str)
            .unwrap_or("warn");
        let reason = thesis
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("thesis_quality_unknown");
        let recommendation = thesis
            .get("recommendation")
            .and_then(Value::as_str)
            .unwrap_or("review_thesis_guard");
        items.push(RecommendationItem {
            recommendation_id: "thesis_guard".to_string(),
            severity: severity.to_string(),
            confidence: "high".to_string(),
            recommendation: recommendation.to_string(),
            evidence: format!("thesis quality reason: {}", reason),
            expected_tradeoff:
                "may reduce long-side thesis losses at the cost of fewer exits staying open"
                    .to_string(),
        });
    }

    if let Some(symbol) = symbols
        .iter()
        .find(|item| item.status == "fragile")
        .or_else(|| symbols.first())
    {
        items.push(RecommendationItem {
            recommendation_id: format!("symbol_{}", symbol.symbol.to_lowercase()),
            severity: if symbol.status == "fragile" {
                "warn".to_string()
            } else {
                "info".to_string()
            },
            confidence: symbol.confidence.clone(),
            recommendation: format!("{} for {}", symbol.recommendation, symbol.symbol),
            evidence: format!(
                "{} trades, avg pnl {:+.4}%, thesis trades {}, trailing trades {}",
                symbol.trades,
                symbol.avg_pnl_pct,
                symbol.thesis_invalidated_trades,
                symbol.trailing_stop_trades
            ),
            expected_tradeoff: "symbol-specific tuning may improve local performance while reducing overall trade count".to_string(),
        });
    }

    if let Some(blocker) = blockers.first() {
        items.push(RecommendationItem {
            recommendation_id: "entry_pressure".to_string(),
            severity: "info".to_string(),
            confidence: "medium".to_string(),
            recommendation: format!("monitor dominant entry gate: {}", blocker.reason),
            evidence: format!("top blocker hit {} times in current window", blocker.total),
            expected_tradeoff:
                "changing blocker thresholds can increase fill rate but may lower setup quality"
                    .to_string(),
        });
    }

    if comparative.status == "regressed" || expectancy.expectancy_pct < 0.0 {
        items.push(RecommendationItem {
            recommendation_id: "regression_guard".to_string(),
            severity: "warn".to_string(),
            confidence: "medium".to_string(),
            recommendation: "prefer observation before broad retuning".to_string(),
            evidence: format!(
                "comparative status={}, expectancy {:+.4}%",
                comparative.status, expectancy.expectancy_pct
            ),
            expected_tradeoff:
                "slower tuning cadence reduces the risk of chasing short-window noise".to_string(),
        });
    } else if summary.closed_trades > 0 {
        items.push(RecommendationItem {
            recommendation_id: "keep_watching".to_string(),
            severity: "info".to_string(),
            confidence: "medium".to_string(),
            recommendation: "keep the current changes running and accumulate more sample"
                .to_string(),
            evidence: format!(
                "{} closed trades, expectancy {:+.4}%, status {}",
                summary.closed_trades, expectancy.expectancy_pct, comparative.status
            ),
            expected_tradeoff: "more sample improves confidence before the next runtime change"
                .to_string(),
        });
    }

    items.truncate(4);
    items
}

fn build_heuristic_summary(ctx: HeuristicSummaryContext<'_>) -> String {
    let worst_symbol = ctx.by_symbol.first();
    let best_symbol = ctx.by_symbol.iter().max_by(|a, b| {
        a.pnl_usdt
            .partial_cmp(&b.pnl_usdt)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let dominant_close = ctx.by_close_reason.first();
    let long_side = ctx
        .by_side
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case("long"));
    let short_side = ctx
        .by_side
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case("short"));
    let top_blocker = ctx.blockers.first();
    let top_thesis = ctx.thesis_breakdown.first();

    format!(
        "Lookback {}h: {} closed trades, total pnl {:+.4} USDT, avg pnl {:+.4}%, win rate {:.2}%. Dominant exit: {} ({} trades, {:+.4}% avg). Long side: {:+.4}% avg over {} trades. Short side: {:+.4}% avg over {} trades. Worst symbol: {} ({:+.4} USDT). Best symbol: {} ({:+.4} USDT). Top entry blocker: {} ({} hits). Top thesis reason: {} ({} hits).",
        ctx.hours,
        ctx.summary.closed_trades,
        ctx.summary.total_pnl_usdt,
        ctx.summary.avg_pnl_pct,
        ctx.summary.win_rate_pct,
        dominant_close
            .map(|item| item.name.as_str())
            .unwrap_or("n/a"),
        dominant_close.map(|item| item.trades).unwrap_or(0),
        dominant_close.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
        long_side.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
        long_side.map(|item| item.trades).unwrap_or(0),
        short_side.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
        short_side.map(|item| item.trades).unwrap_or(0),
        worst_symbol.map(|item| item.name.as_str()).unwrap_or("n/a"),
        worst_symbol.map(|item| item.pnl_usdt).unwrap_or(0.0),
        best_symbol.map(|item| item.name.as_str()).unwrap_or("n/a"),
        best_symbol.map(|item| item.pnl_usdt).unwrap_or(0.0),
        top_blocker
            .map(|item| item.reason.as_str())
            .unwrap_or("n/a"),
        top_blocker.map(|item| item.total).unwrap_or(0),
        top_thesis
            .map(|item| item.reason.as_str())
            .unwrap_or("n/a"),
        top_thesis.map(|item| item.total).unwrap_or(0),
    ) + &format!(
        " Expectancy {:+.4} USDT / {:+.4}% per trade, payoff ratio {:.2}, avg win {:+.4}% vs avg loss {:+.4}%.",
        ctx.expectancy.expectancy_usdt,
        ctx.expectancy.expectancy_pct,
        ctx.expectancy.payoff_ratio,
        ctx.expectancy.avg_win_pct,
        ctx.expectancy.avg_loss_pct,
    )
}

fn build_tupa_snapshot(ctx: TupaSnapshotContext<'_>) -> AnalystSnapshot {
    let total_trades = ctx.summary.closed_trades.max(1) as f64;
    let thesis_invalidated = ctx
        .by_close_reason
        .iter()
        .find(|item| item.name == "thesis_invalidated");
    let trailing_stop = ctx
        .by_close_reason
        .iter()
        .find(|item| item.name == "trailing_stop");
    let long_side = ctx
        .by_side
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case("long"));
    let short_side = ctx
        .by_side
        .iter()
        .find(|item| item.name.eq_ignore_ascii_case("short"));
    let worst_symbol = ctx.by_symbol.first();
    let best_symbol = ctx.by_symbol.iter().max_by(|a, b| {
        a.pnl_usdt
            .partial_cmp(&b.pnl_usdt)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let consensus_blocks = ctx
        .blockers
        .iter()
        .filter(|item| item.reason.contains("consensus"))
        .map(|item| item.total)
        .sum();
    let volume_blocks = ctx
        .blockers
        .iter()
        .filter(|item| item.reason.contains("volume"))
        .map(|item| item.total)
        .sum();
    let macd_blocks = ctx
        .blockers
        .iter()
        .filter(|item| item.reason.contains("macd"))
        .map(|item| item.total)
        .sum();
    let top_thesis_reason = ctx
        .thesis_breakdown
        .first()
        .map(|item| item.reason.clone())
        .unwrap_or_else(|| "n/a".to_string());
    let top_thesis_reason_hits = ctx
        .thesis_breakdown
        .first()
        .map(|item| item.total)
        .unwrap_or(0);
    let no_alignment_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| {
            item.reason.contains("no_bullish_alignment")
                || item.reason.contains("no_bearish_alignment")
        })
        .map(|item| item.total)
        .sum();
    let health_threshold_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| item.reason.contains("health_threshold"))
        .map(|item| item.total)
        .sum();
    let opposite_side_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| item.reason.contains("opposite_side"))
        .map(|item| item.total)
        .sum();
    let consensus_trend_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| item.reason.contains("consensus_trend_score"))
        .map(|item| item.total)
        .sum();
    let price_vs_fast_ema_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| item.reason.contains("price_vs_fast_ema"))
        .map(|item| item.total)
        .sum();
    let btc_regime_hits = ctx
        .thesis_breakdown
        .iter()
        .filter(|item| item.reason.contains("btc_regime"))
        .map(|item| item.total)
        .sum();

    AnalystSnapshot {
        lookback_hours: ctx.hours,
        summary: SnapshotSummary {
            closed_trades: ctx.summary.closed_trades,
            total_pnl_usdt: ctx.summary.total_pnl_usdt,
            avg_pnl_pct: ctx.summary.avg_pnl_pct,
            avg_duration_s: ctx.summary.avg_duration_s,
            win_rate_pct: ctx.summary.win_rate_pct,
        },
        expectancy: ExpectancyMetrics {
            winning_trades: ctx.expectancy.winning_trades,
            losing_trades: ctx.expectancy.losing_trades,
            neutral_trades: ctx.expectancy.neutral_trades,
            avg_win_usdt: ctx.expectancy.avg_win_usdt,
            avg_win_pct: ctx.expectancy.avg_win_pct,
            avg_loss_usdt: ctx.expectancy.avg_loss_usdt,
            avg_loss_pct: ctx.expectancy.avg_loss_pct,
            payoff_ratio: ctx.expectancy.payoff_ratio,
            expectancy_usdt: ctx.expectancy.expectancy_usdt,
            expectancy_pct: ctx.expectancy.expectancy_pct,
        },
        exits: SnapshotExitMetrics {
            thesis_invalidated_pct: thesis_invalidated
                .map(|item| 100.0 * item.trades as f64 / total_trades)
                .unwrap_or(0.0),
            thesis_invalidated_avg_pnl_pct: thesis_invalidated
                .map(|item| item.avg_pnl_pct)
                .unwrap_or(0.0),
            trailing_stop_pct: trailing_stop
                .map(|item| 100.0 * item.trades as f64 / total_trades)
                .unwrap_or(0.0),
            trailing_stop_avg_pnl_pct: trailing_stop.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
        },
        sides: SnapshotSideMetrics {
            long_trade_share_pct: long_side
                .map(|item| 100.0 * item.trades as f64 / total_trades)
                .unwrap_or(0.0),
            short_trade_share_pct: short_side
                .map(|item| 100.0 * item.trades as f64 / total_trades)
                .unwrap_or(0.0),
            long_avg_pnl_pct: long_side.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
            short_avg_pnl_pct: short_side.map(|item| item.avg_pnl_pct).unwrap_or(0.0),
        },
        blockers: SnapshotBlockerMetrics {
            top_reason: ctx
                .blockers
                .first()
                .map(|item| item.reason.clone())
                .unwrap_or_else(|| "n/a".to_string()),
            top_reason_hits: ctx.blockers.first().map(|item| item.total).unwrap_or(0),
            consensus_blocks,
            volume_blocks,
            macd_blocks,
        },
        thesis: SnapshotThesisMetrics {
            total_closes: ctx.thesis_summary.total_closes,
            top_reason: top_thesis_reason,
            top_reason_hits: top_thesis_reason_hits,
            positive_close_pct: if ctx.thesis_summary.total_closes > 0 {
                100.0 * ctx.thesis_summary.positive_closes as f64
                    / ctx.thesis_summary.total_closes as f64
            } else {
                0.0
            },
            long_avg_pnl_pct: ctx.thesis_summary.long_avg_pnl_pct,
            short_avg_pnl_pct: ctx.thesis_summary.short_avg_pnl_pct,
            no_alignment_hits,
            health_threshold_hits,
            opposite_side_hits,
            consensus_trend_hits,
            price_vs_fast_ema_hits,
            btc_regime_hits,
        },
        symbols: SnapshotSymbolMetrics {
            worst_symbol: worst_symbol
                .map(|item| item.name.clone())
                .unwrap_or_else(|| "n/a".to_string()),
            worst_symbol_pnl_usdt: worst_symbol.map(|item| item.pnl_usdt).unwrap_or(0.0),
            best_symbol: best_symbol
                .map(|item| item.name.clone())
                .unwrap_or_else(|| "n/a".to_string()),
            best_symbol_pnl_usdt: best_symbol.map(|item| item.pnl_usdt).unwrap_or(0.0),
        },
    }
}

async fn request_llm_summary(
    state: &AppState,
    heuristic_summary: &str,
) -> Result<Option<String>, AnalystError> {
    let Some(base_url) = &state.ollama_url else {
        return Ok(None);
    };
    let Some(model) = &state.ollama_model else {
        return Ok(None);
    };

    let prompt = format!(
        "You are an AI analyst for a crypto trading bot. Summarize the latest diagnostics in a short operational note, highlighting risk, stability, and next steps. Data: {}",
        heuristic_summary
    );

    let response = state
        .http_client
        .post(format!("{}/api/generate", base_url.trim_end_matches('/')))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let body: serde_json::Value = response.json().await?;
    Ok(body
        .get("response")
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string()))
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    if err.is_not_found() {
        let reply = warp::reply::with_status(
            warp::reply::json(&ApiError {
                error: "not_found",
                message: "route not found".to_string(),
            }),
            StatusCode::NOT_FOUND,
        );
        return Ok(reply);
    }

    let reply = warp::reply::with_status(
        warp::reply::json(&ApiError {
            error: "internal_error",
            message: "request failed".to_string(),
        }),
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    Ok(reply)
}
