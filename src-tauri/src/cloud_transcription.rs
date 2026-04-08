use crate::settings::{AppSettings, CloudTranscriptionProvider};
use anyhow::{anyhow, Result};
use base64::Engine;
use log::warn;
use serde::Deserialize;
use std::collections::HashSet;

const OPENAI_TRANSCRIPTION_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
const GROQ_TRANSCRIPTION_URL: &str = "https://api.groq.com/openai/v1/audio/transcriptions";

#[derive(Debug, Deserialize)]
struct OpenAiVerboseSegment {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiVerboseJson {
    text: Option<String>,
    segments: Option<Vec<OpenAiVerboseSegment>>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Debug, Deserialize)]
struct GeminiContent {
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
struct GeminiPart {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiGenerateResponse {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Debug, Deserialize)]
struct ModelsListItem {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Option<Vec<ModelsListItem>>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModelEntry>>,
}

#[derive(Debug, Deserialize)]
struct GeminiModelEntry {
    name: Option<String>,
    supported_generation_methods: Option<Vec<String>>,
}

fn whisper_language_param(code: &str) -> Option<String> {
    match code {
        "auto" => None,
        "zh-Hans" | "zh-Hant" => Some("zh".to_string()),
        _ => Some(code.to_string()),
    }
}

fn text_from_verbose_json(body: &str) -> Result<String> {
    let parsed: OpenAiVerboseJson = serde_json::from_str(body)
        .map_err(|e| anyhow!("Invalid transcription JSON: {}", e))?;
    if let Some(t) = parsed.text {
        let t = t.trim().to_string();
        if !t.is_empty() {
            return Ok(t);
        }
    }
    if let Some(segments) = parsed.segments {
        let joined: String = segments
            .iter()
            .filter_map(|s| s.text.as_ref().map(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join("");
        let joined = joined.trim().to_string();
        if !joined.is_empty() {
            return Ok(joined);
        }
    }
    Err(anyhow!("Transcription response had no text"))
}

fn effective_cloud_provider(settings: &AppSettings) -> CloudTranscriptionProvider {
    match settings.selected_model.as_str() {
        "cloud-openai" => CloudTranscriptionProvider::OpenAI,
        "cloud-groq" => CloudTranscriptionProvider::Groq,
        "cloud-gemini" => CloudTranscriptionProvider::Gemini,
        _ => settings.cloud_transcription_provider,
    }
}

pub fn transcribe_cloud_with_settings(settings: &AppSettings, wav_bytes: Vec<u8>) -> Result<String> {
    let provider = effective_cloud_provider(settings);
    let model = settings
        .cloud_transcription_models
        .get(&cloud_provider_key(provider))
        .cloned()
        .unwrap_or_default();
    let model = model.trim();
    if model.is_empty() {
        return Err(anyhow!("No cloud transcription model selected"));
    }

    let api_key = settings
        .cloud_transcription_api_keys
        .get(&cloud_provider_key(provider))
        .cloned()
        .unwrap_or_default();
    if api_key.trim().is_empty() {
        return Err(anyhow!(
            "No API key configured for {}",
            cloud_provider_label(provider)
        ));
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| anyhow!("HTTP client: {}", e))?;

    match provider {
        CloudTranscriptionProvider::OpenAI => transcribe_openai_compat(
            &client,
            OPENAI_TRANSCRIPTION_URL,
            &api_key,
            model,
            &wav_bytes,
            settings,
        ),
        CloudTranscriptionProvider::Groq => transcribe_openai_compat(
            &client,
            GROQ_TRANSCRIPTION_URL,
            &api_key,
            model,
            &wav_bytes,
            settings,
        ),
        CloudTranscriptionProvider::Gemini => {
            transcribe_gemini(&client, &api_key, model, &wav_bytes, settings)
        }
    }
}

fn cloud_provider_key(p: CloudTranscriptionProvider) -> String {
    match p {
        CloudTranscriptionProvider::OpenAI => "openai".to_string(),
        CloudTranscriptionProvider::Groq => "groq".to_string(),
        CloudTranscriptionProvider::Gemini => "gemini".to_string(),
    }
}

fn cloud_provider_label(p: CloudTranscriptionProvider) -> &'static str {
    match p {
        CloudTranscriptionProvider::OpenAI => "OpenAI",
        CloudTranscriptionProvider::Groq => "Groq",
        CloudTranscriptionProvider::Gemini => "Gemini",
    }
}

fn transcribe_openai_compat(
    client: &reqwest::blocking::Client,
    url: &str,
    api_key: &str,
    model: &str,
    wav_bytes: &[u8],
    settings: &AppSettings,
) -> Result<String> {
    let part = reqwest::blocking::multipart::Part::bytes(wav_bytes.to_vec())
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| anyhow!("multipart: {}", e))?;

    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string())
        .text("response_format", "verbose_json".to_string())
        .text("temperature", "0".to_string());

    if let Some(lang) = whisper_language_param(settings.selected_language.as_str()) {
        form = form.text("language", lang);
    }

    let resp = client
        .post(url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key.trim()),
        )
        .multipart(form)
        .send()
        .map_err(|e| anyhow!("Transcription request failed: {}", e))?;

    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| anyhow!("Reading response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow!(
            "Transcription API error ({}): {}",
            status.as_u16(),
            body.trim()
        ));
    }

    text_from_verbose_json(&body)
}

fn transcribe_gemini(
    client: &reqwest::blocking::Client,
    api_key: &str,
    model: &str,
    wav_bytes: &[u8],
    settings: &AppSettings,
) -> Result<String> {
    let model_path = if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/{}:generateContent?key={}",
        model_path,
        urlencoding::encode(api_key.trim())
    );

    let b64 = base64::engine::general_purpose::STANDARD.encode(wav_bytes);

    let lang_hint = if settings.selected_language == "auto" {
        String::new()
    } else {
        format!(
            " The spoken language is likely {} (BCP-47 / app code).",
            settings.selected_language
        )
    };

    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                {
                    "inline_data": {
                        "mime_type": "audio/wav",
                        "data": b64
                    }
                },
                {
                    "text": format!(
                        "Transcribe the speech in this audio to plain text only. No timestamps, no speaker labels, no markdown.{}",
                        lang_hint
                    )
                }
            ]
        }]
    });

    let resp = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .map_err(|e| anyhow!("Gemini request failed: {}", e))?;

    let status = resp.status();
    let resp_text = resp
        .text()
        .map_err(|e| anyhow!("Reading Gemini response: {}", e))?;

    if !status.is_success() {
        return Err(anyhow!(
            "Gemini API error ({}): {}",
            status.as_u16(),
            resp_text.trim()
        ));
    }

    let parsed: GeminiGenerateResponse = serde_json::from_str(&resp_text)
        .map_err(|e| anyhow!("Invalid Gemini JSON: {}", e))?;

    let text = parsed
        .candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|c| c.parts.as_ref())
        .and_then(|parts| {
            let s: String = parts
                .iter()
                .filter_map(|p| p.text.as_ref())
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("");
            let t = s.trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        });

    text.ok_or_else(|| anyhow!("Gemini returned no text"))
}

pub fn fetch_cloud_transcription_models_sync(
    provider: CloudTranscriptionProvider,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API key is required to list models".to_string());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    match provider {
        CloudTranscriptionProvider::OpenAI => fetch_openai_style_audio_models(
            &client,
            "https://api.openai.com/v1/models",
            key,
        ),
        CloudTranscriptionProvider::Groq => {
            fetch_openai_style_audio_models(&client, "https://api.groq.com/openai/v1/models", key)
        }
        CloudTranscriptionProvider::Gemini => fetch_gemini_models(&client, key),
    }
}

fn fetch_openai_style_audio_models(
    client: &reqwest::blocking::Client,
    list_url: &str,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let resp = client
        .get(list_url)
        .header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", api_key.trim()),
        )
        .send()
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let body = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Models API ({}): {}", status.as_u16(), body));
    }

    let parsed: OpenAiModelsResponse =
        serde_json::from_str(&body).map_err(|e| format!("Invalid models JSON: {}", e))?;

    let mut ids: Vec<String> = parsed
        .data
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| m.id.or(m.name))
        .filter(|id| {
            let lower = id.to_lowercase();
            lower.contains("whisper")
                || lower.contains("gpt-4o-transcribe")
                || lower.contains("gpt-4o-mini-transcribe")
        })
        .collect();

    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn fetch_gemini_models(
    client: &reqwest::blocking::Client,
    api_key: &str,
) -> Result<Vec<String>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        urlencoding::encode(api_key.trim())
    );

    let resp = client.get(&url).send().map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Gemini models ({}): {}", status.as_u16(), body));
    }

    let parsed: GeminiModelsResponse =
        serde_json::from_str(&body).map_err(|e| format!("Invalid Gemini models JSON: {}", e))?;

    let allowed: HashSet<&'static str> = [
        "generateContent",
        "countTokens",
        "createCachedContent",
        "batchGenerateContent",
    ]
    .into_iter()
    .collect();

    let mut out: Vec<String> = parsed
        .models
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            let methods = m.supported_generation_methods.unwrap_or_default();
            if !methods.iter().any(|x| allowed.contains(x.as_str())) {
                return None;
            }
            let name = m.name?;
            let short = name.strip_prefix("models/").unwrap_or(&name).to_string();
            let lower = short.to_lowercase();
            if lower.contains("embedding") || lower.contains("embed") {
                return None;
            }
            if lower.contains("tts") || lower.contains("text-to-speech") {
                return None;
            }
            Some(short)
        })
        .collect();

    out.sort();
    out.dedup();

    if out.is_empty() {
        warn!("Gemini models list empty after filter; using defaults");
        Ok(default_gemini_models())
    } else {
        Ok(out)
    }
}

fn default_gemini_models() -> Vec<String> {
    vec![
        "gemini-2.0-flash".to_string(),
        "gemini-2.5-flash-preview-05-20".to_string(),
        "gemini-1.5-flash".to_string(),
        "gemini-1.5-pro".to_string(),
    ]
}
