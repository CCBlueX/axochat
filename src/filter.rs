use crate::config::{Filter, FilterConfig, Threshold};

use rustrict::{Censor, Type};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_PROMPT: &str = include_str!("filter.prompt.md");

/// The filters for global messages of users who are not staff: rustrict censors before delivery,
/// screens take a message back after it was delivered.
pub struct Filters {
    censor: Option<Type>,
    pub screens: Vec<Arc<Screen>>,
}

pub enum Screen {
    Moderation(ModerationApi),
    Llm(Llm),
}

pub struct ModerationApi {
    client: reqwest::Client,
    url: String,
    key: String,
    model: String,
    thresholds: Vec<Threshold>,
}

pub struct Llm {
    client: reqwest::Client,
    url: String,
    key: Option<String>,
    model: String,
    prompt: String,
}

impl Filters {
    pub fn new(config: &FilterConfig) -> Result<Filters, String> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("axochat/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("could not build http client");
        let mut filters = Filters { censor: None, screens: Vec::new() };
        let mut chosen = config.filters.clone();
        chosen.sort();
        chosen.dedup();
        for filter in chosen {
            match filter {
                Filter::None => {}
                Filter::Rustrict => {
                    filters.censor = Some(config.censor.iter().fold(Type::NONE, |all, level| all | level.0));
                }
                Filter::Moderation => filters.screens.push(Arc::new(Screen::Moderation(ModerationApi {
                    client: client.clone(),
                    url: format!("{}/moderations", config.moderation_url.trim_end_matches('/')),
                    key: config.moderation_key.clone().ok_or("the moderation filter needs OPENAI_API_KEY")?,
                    model: config.moderation_model.clone(),
                    thresholds: config.thresholds.clone(),
                }))),
                Filter::Llm => {
                    let prompt = match &config.llm_prompt {
                        Some(path) => std::fs::read_to_string(path)
                            .map_err(|err| format!("could not read {}: {}", path.display(), err))?,
                        None => DEFAULT_PROMPT.to_owned(),
                    };
                    filters.screens.push(Arc::new(Screen::Llm(Llm {
                        client: client.clone(),
                        url: format!("{}/chat/completions", config.llm_url.trim_end_matches('/')),
                        key: config.llm_key.clone(),
                        model: config.llm_model.clone().ok_or("the llm filter needs LLM_MODEL")?,
                        prompt,
                    })));
                }
            }
        }
        Ok(filters)
    }

    pub fn names(&self) -> Vec<&'static str> {
        let censor = self.censor.map(|_| "rustrict");
        censor.into_iter().chain(self.screens.iter().map(|screen| screen.name())).collect()
    }

    /// The message as it is delivered, if rustrict censored some of it.
    pub fn censor(&self, text: &str) -> Option<String> {
        let threshold = self.censor?;
        let mut censor = Censor::from_str(text);
        censor.with_censor_threshold(threshold);
        let (censored, analysis) = censor.censor_and_analyze();
        analysis.is(threshold).then_some(censored)
    }
}

impl Screen {
    pub fn name(&self) -> &'static str {
        match self {
            Screen::Moderation(_) => "moderation",
            Screen::Llm(_) => "llm",
        }
    }

    /// Why the message should be taken back, if it should.
    pub async fn violation(&self, text: &str) -> Result<Option<String>, reqwest::Error> {
        match self {
            Screen::Moderation(api) => api.violation(text).await,
            Screen::Llm(llm) => llm.violation(text).await,
        }
    }
}

#[derive(Serialize)]
struct ModerationRequest<'a> {
    model: &'a str,
    input: &'a str,
}

#[derive(Deserialize)]
struct ModerationResponse {
    results: Vec<Scores>,
}

#[derive(Deserialize)]
struct Scores {
    category_scores: HashMap<String, f64>,
}

impl ModerationApi {
    async fn violation(&self, text: &str) -> Result<Option<String>, reqwest::Error> {
        let response: ModerationResponse = self
            .client
            .post(&self.url)
            .bearer_auth(&self.key)
            .json(&ModerationRequest { model: &self.model, input: text })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response
            .results
            .first()
            .and_then(|result| violation(&result.category_scores, &self.thresholds))
            .map(str::to_owned))
    }
}

/// The category furthest over its threshold.
fn violation<'a>(scores: &HashMap<String, f64>, thresholds: &'a [Threshold]) -> Option<&'a str> {
    thresholds
        .iter()
        .filter_map(|threshold| {
            let score = *scores.get(&threshold.category)?;
            (score >= threshold.score).then_some((threshold.category.as_str(), score / threshold.score))
        })
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(category, _)| category)
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage<'a>; 2],
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Reply,
}

#[derive(Deserialize)]
struct Reply {
    content: Option<String>,
}

impl Llm {
    async fn violation(&self, text: &str) -> Result<Option<String>, reqwest::Error> {
        let messages = [
            ChatMessage { role: "system", content: &self.prompt },
            ChatMessage { role: "user", content: text },
        ];
        // OpenCode Go refuses requests without a session, and caches the system prompt per session
        let mut request = self
            .client
            .post(&self.url)
            .header("x-opencode-session", "axochat-filter")
            .json(&ChatRequest { model: &self.model, messages });
        if let Some(key) = &self.key {
            request = request.bearer_auth(key);
        }
        let response: ChatResponse = request.send().await?.error_for_status()?.json().await?;
        let answer = response.choices.into_iter().next().and_then(|choice| choice.message.content);
        Ok(answer.filter(|answer| removes(answer)).map(|_| "slur or hate".to_owned()))
    }
}

/// Anything but a leading `REMOVE` keeps the message, also a reply that ignored the format.
fn removes(answer: &str) -> bool {
    answer.trim_start().get(..6).is_some_and(|word| word.eq_ignore_ascii_case("remove"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CensorLevel;

    #[test]
    fn only_listed_categories_over_their_threshold_count() {
        let thresholds: Vec<Threshold> = ["hate=0.5", "sexual/minors=0.2"].iter().map(|t| t.parse().unwrap()).collect();
        let scores = |pairs: &[(&str, f64)]| pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect();

        assert_eq!(violation(&scores(&[("harassment", 0.99), ("hate", 0.49)]), &thresholds), None);
        assert_eq!(violation(&scores(&[("hate", 0.5)]), &thresholds), Some("hate"));
        assert_eq!(
            violation(&scores(&[("hate", 0.9), ("sexual/minors", 0.5)]), &thresholds),
            Some("sexual/minors"),
            "2.5 times its threshold beats 1.8 times"
        );
    }

    #[test]
    fn llm_answers() {
        assert!(removes("REMOVE"));
        assert!(removes(" remove."));
        assert!(!removes("KEEP"));
        assert!(!removes("I'm ready to review the message"));
        assert!(!removes(""));
    }

    #[test]
    fn rustrict_censors_slurs_and_leaves_banter() {
        let filters = Filters {
            censor: Some("offensive:severe".parse::<CensorLevel>().unwrap().0),
            screens: Vec::new(),
        };
        assert_eq!(filters.censor("shut up nigger").as_deref(), Some("shut up ******"));
        for banter in ["ur gay lol", "kys", "bro is retarded", "für dich", "fuck this anticheat"] {
            assert_eq!(filters.censor(banter), None, "{}", banter);
        }
    }
}
