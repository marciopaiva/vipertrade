//! OpenRouter narration for `POST /analyze/tuning` (Format A). The LLM ONLY writes
//! prose over numbers the deterministic grid already computed — it never builds an
//! override path, runs a sweep, or invents a PnL. The recommendation is decided in
//! Rust (`TuningGridResult.recommended`, best positive-delta alpha) and handed in;
//! the model narrates it. Narration is best-effort: callers fall back to the raw grid
//! if it fails, so the deterministic result is always available.

use crate::AnalystError;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

const SYSTEM_PROMPT: &str = "Você é o analista de tuning do ViperTrade (copy-trade Bybit, \
modo PAPER). Recebe o resultado de um BACKTEST DETERMINÍSTICO já calculado (grid de \
variantes + performance por símbolo). Os números são FINAIS: nunca recalcule, nunca \
invente PnL. Você é a CAMADA DE COMENTÁRIO: NUNCA emita paths de config, valores, nem \
diffs/blocos de pairs.yaml — a UI já mostra o diff autoritativo separadamente; seu papel \
é só explicar em prosa. Escreva um relatório CONCISO em pt-BR (markdown):\n\
- Baseline: net_pnl, win-rate, principais close_reasons e piores símbolos.\n\
- Tabela das variantes ordenada por delta_net_pnl, com SINAL explícito. NUNCA apresente \
um delta negativo como ganho.\n\
- Classifique cada variante pelo campo `class`: 'alpha' (muda a estrutura de entrada/saída) \
vs 'exposure' (só reduz tamanho — num book net-negativo isso 'melhora' por reduzir \
exposição, NÃO é alpha; não recomende como tuning).\n\
- Recomendação: refira-se EXATAMENTE à variante do campo `recommended` (já escolhida pelo \
backend como o melhor alpha com delta positivo) pelo nome do eixo, valor e delta, em prosa. \
Se `recommended` for null, diga que não houve melhoria de alpha no corpus atual. NÃO escreva \
um diff — a UI já o exibe.\n\
- A SUBSTITUIÇÃO DE TOKEN é um eixo SEPARADO e independente da recomendação de parâmetro: \
NUNCA misture os dois. Os candidatos do pool NÃO têm corpus, então trate a troca como \
HIPÓTESE a validar em paper, jamais como ganho de PnL comprovado, e jamais como o diff da \
recomendação.\n\
Responda somente com o markdown do relatório, sem blocos de código de config.";

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Deserialize)]
struct Message {
    content: String,
}

/// Narrate the deterministic grid result. `payload` is the serialized
/// `TuningGridResult` JSON. Returns the markdown report.
pub async fn narrate(
    client: &Client,
    api_key: &str,
    model: &str,
    payload: &str,
) -> Result<String, AnalystError> {
    let body = json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": payload },
        ],
        "temperature": 0.2,
    });

    let response: ChatResponse = client
        .post(OPENROUTER_URL)
        .bearer_auth(api_key)
        // OpenRouter ranking/attribution headers (optional but recommended).
        .header("HTTP-Referer", "https://github.com/marciopaiva/vipertrade")
        .header("X-Title", "ViperTrade")
        // Bound a slow/hung free-tier model so it can't hang the whole endpoint —
        // on timeout the caller falls back to the cached narration.
        .timeout(std::time::Duration::from_secs(90))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    response
        .choices
        .into_iter()
        .next()
        .map(|choice| choice.message.content)
        .ok_or_else(|| AnalystError::Runtime("OpenRouter returned no choices".to_string()))
}
