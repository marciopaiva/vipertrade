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
invente PnL, nunca monte paths de config. Escreva um relatório CONCISO em pt-BR (markdown):\n\
- Baseline: net_pnl, win-rate, principais close_reasons e piores símbolos.\n\
- Tabela das variantes ordenada por delta_net_pnl, com SINAL explícito. NUNCA apresente \
um delta negativo como ganho.\n\
- Classifique cada variante pelo campo `class`: 'alpha' (muda a estrutura de entrada/saída) \
vs 'exposure' (só reduz tamanho — num book net-negativo isso 'melhora' por reduzir \
exposição, NÃO é alpha; não recomende como tuning).\n\
- Performance por token e a hipótese de substituição: os candidatos do pool NÃO têm \
corpus, então trate a troca como HIPÓTESE a validar, jamais como ganho de PnL comprovado.\n\
- Recomendação: use EXATAMENTE a variante em `recommended` (já escolhida pelo backend como \
o melhor alpha com delta positivo). Se `recommended` for null, diga que não houve melhoria \
de alpha no corpus atual. Inclua o diff do pairs.yaml a aplicar.\n\
Responda somente com o markdown do relatório.";

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
