use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Clone)]
struct AppConfig {
  ollama_host: String,
}

#[derive(Deserialize)]
struct TagModel {
  name: String,
}

#[derive(Deserialize)]
struct TagsResponse {
  #[serde(default)]
  models: Vec<TagModel>,
}

#[derive(Serialize)]
struct GenerateOptions {
  temperature: f32,
}

#[derive(Serialize)]
struct GenerateRequest {
  model: String,
  prompt: String,
  stream: bool,
  options: GenerateOptions,
}

#[derive(Deserialize)]
struct GenerateResponse {
  response: Option<String>,
}

fn make_prompt(transcript: &str, instruction: &str) -> String {
  [
    instruction,
    "",
    "Raw transcript:",
    transcript,
    "",
    "Output only the cleaned dictation text.",
  ]
  .join("\n")
}

fn normalize_host(host: &str) -> String {
  host.trim_end_matches('/').to_string()
}

#[tauri::command]
async fn list_models(state: State<'_, AppConfig>) -> Result<Vec<String>, String> {
  let client = Client::new();
  let url = format!("{}/api/tags", normalize_host(&state.ollama_host));

  let response = client
    .get(url)
    .send()
    .await
    .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

  if !response.status().is_success() {
    return Err(format!("Ollama /api/tags returned {}", response.status()));
  }

  let payload = response
    .json::<TagsResponse>()
    .await
    .map_err(|e| format!("Invalid Ollama response: {e}"))?;

  Ok(payload.models.into_iter().map(|m| m.name).collect())
}

#[tauri::command]
async fn refine_dictation(
  model: String,
  transcript: String,
  instruction: Option<String>,
  state: State<'_, AppConfig>,
) -> Result<String, String> {
  let trimmed_model = model.trim().to_string();
  let trimmed_transcript = transcript.trim().to_string();

  if trimmed_model.is_empty() {
    return Err("Missing model".to_string());
  }

  if trimmed_transcript.is_empty() {
    return Err("Missing transcript".to_string());
  }

  let prompt_instruction = instruction
    .as_deref()
    .map(str::trim)
    .filter(|v| !v.is_empty())
    .unwrap_or("Clean up this raw speech-to-text transcript into readable text while preserving the speaker's intent.");

  let payload = GenerateRequest {
    model: trimmed_model,
    prompt: make_prompt(&trimmed_transcript, prompt_instruction),
    stream: false,
    options: GenerateOptions { temperature: 0.2 },
  };

  let client = Client::new();
  let url = format!("{}/api/generate", normalize_host(&state.ollama_host));

  let response = client
    .post(url)
    .json(&payload)
    .send()
    .await
    .map_err(|e| format!("Failed to reach Ollama: {e}"))?;

  if !response.status().is_success() {
    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| "<no body>".to_string());
    return Err(format!("Ollama /api/generate failed ({status}): {body}"));
  }

  let generated = response
    .json::<GenerateResponse>()
    .await
    .map_err(|e| format!("Invalid Ollama response: {e}"))?;

  Ok(generated.response.unwrap_or_default())
}

fn main() {
  let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

  tauri::Builder::default()
    .manage(AppConfig { ollama_host })
    .invoke_handler(tauri::generate_handler![list_models, refine_dictation])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
