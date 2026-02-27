use std::collections::{hash_map::Entry, HashMap, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sparklytics_core::analytics::{BotClassification, BotPolicyMode};
const BEHAVIOR_WINDOW: Duration = Duration::from_secs(30);
const BURST_EVENT_THRESHOLD: usize = 40;
const PATH_SWEEP_THRESHOLD: usize = 25;
const ACTIVITY_SHARDS: usize = 32;

#[derive(Debug, Clone)]
pub enum BotOverrideDecision {
    ForceHuman,
    ForceBot,
}

#[derive(Debug, Clone)]
pub struct BotPolicyInput {
    pub mode: BotPolicyMode,
    pub threshold_score: i32,
}

#[derive(Debug)]
struct ActivitySample {
    at: Instant,
    path: String,
}

type ActivityKey = (String, String);
type ActivityQueue = VecDeque<ActivitySample>;
type ActivityShardMap = HashMap<ActivityKey, ActivityQueue>;
type ActivityShards = [Mutex<ActivityShardMap>; ACTIVITY_SHARDS];

fn activity_shards() -> &'static ActivityShards {
    static SHARDS: OnceLock<ActivityShards> = OnceLock::new();
    SHARDS.get_or_init(|| std::array::from_fn(|_| Mutex::new(HashMap::new())))
}

fn activity_shard_index(website_id: &str, visitor_id: &str) -> usize {
    let mut hasher = DefaultHasher::new();
    website_id.hash(&mut hasher);
    visitor_id.hash(&mut hasher);
    (hasher.finish() as usize) % ACTIVITY_SHARDS
}

fn has_path_sweep(queue: &VecDeque<ActivitySample>) -> bool {
    let mut unique_paths: Vec<&str> = Vec::with_capacity(PATH_SWEEP_THRESHOLD);
    for sample in queue {
        let path = sample.path.as_str();
        if unique_paths.contains(&path) {
            continue;
        }
        unique_paths.push(path);
        if unique_paths.len() >= PATH_SWEEP_THRESHOLD {
            return true;
        }
    }
    false
}

fn normalize_path(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    let candidate = if let Some(scheme_idx) = without_query.find("://") {
        let rest = &without_query[(scheme_idx + 3)..];
        if let Some(path_idx) = rest.find('/') {
            &rest[path_idx..]
        } else {
            "/"
        }
    } else {
        without_query
    };
    if candidate.is_empty() {
        "/".to_string()
    } else {
        candidate.to_lowercase()
    }
}

fn evaluate_behavior(website_id: &str, visitor_id: &str, url: &str) -> (bool, bool) {
    let key = (website_id.to_string(), visitor_id.to_string());
    let now = Instant::now();
    let path = normalize_path(url);
    let shard = &activity_shards()[activity_shard_index(website_id, visitor_id)];
    let mut map = shard
        .lock()
        .expect("bot detection activity shard mutex poisoned");
    match map.entry(key) {
        Entry::Occupied(mut occupied) => {
            let queue = occupied.get_mut();
            while let Some(front) = queue.front() {
                if now.duration_since(front.at) > BEHAVIOR_WINDOW {
                    queue.pop_front();
                } else {
                    break;
                }
            }

            if queue.is_empty() {
                occupied.remove_entry();
                return (false, false);
            }

            queue.push_back(ActivitySample { at: now, path });
            if queue.len() > 512 {
                queue.pop_front();
            }

            let burst_rate = queue.len() >= BURST_EVENT_THRESHOLD;
            let path_sweep = has_path_sweep(queue);
            (burst_rate, path_sweep)
        }
        Entry::Vacant(vacant) => {
            let mut queue = VecDeque::new();
            queue.push_back(ActivitySample { at: now, path });
            vacant.insert(queue);
            (false, false)
        }
    }
}

fn ua_signature_score(user_agent: &str) -> Option<i32> {
    let ua = user_agent.to_ascii_lowercase();
    let signatures = [
        "bot",
        "spider",
        "crawler",
        "googlebot",
        "bingbot",
        "duckduckbot",
        "yandexbot",
        "baiduspider",
        "ahrefsbot",
        "semrushbot",
        "mj12bot",
        "headlesschrome",
        "phantomjs",
        "python-requests",
        "curl/",
        "wget/",
        "go-http-client",
        "libwww-perl",
        "urllib",
        "httpclient",
    ];
    if signatures.iter().any(|sig| ua.contains(sig)) {
        Some(90)
    } else {
        None
    }
}

fn policy_threshold(policy: &BotPolicyInput) -> i32 {
    policy.threshold_score.clamp(0, 100)
}

#[allow(clippy::too_many_arguments)]
pub fn classify_event(
    website_id: &str,
    visitor_id: &str,
    url: &str,
    user_agent: &str,
    has_accept_header: bool,
    has_accept_language_header: bool,
    policy: &BotPolicyInput,
    override_decision: Option<BotOverrideDecision>,
) -> BotClassification {
    if let Some(decision) = override_decision {
        return match decision {
            BotOverrideDecision::ForceHuman => BotClassification {
                is_bot: false,
                bot_score: 0,
                bot_reason: Some("allowlist".to_string()),
            },
            BotOverrideDecision::ForceBot => BotClassification {
                is_bot: true,
                bot_score: 100,
                bot_reason: Some("blocklist".to_string()),
            },
        };
    }

    let mut score = 0_i32;
    let mut primary_reason: Option<(&str, i32)> = None;
    let mut bump = |reason: &'static str, value: i32| {
        score = (score + value).clamp(0, 100);
        if primary_reason
            .map(|(_, existing)| value > existing)
            .unwrap_or(true)
        {
            primary_reason = Some((reason, value));
        }
    };

    if let Some(value) = ua_signature_score(user_agent) {
        bump("ua_signature", value);
    }

    if !has_accept_header || !has_accept_language_header {
        bump("header_anomaly", 25);
    }

    let (burst_rate, path_sweep) = evaluate_behavior(website_id, visitor_id, url);
    if burst_rate {
        bump("burst_rate", 40);
    }
    if path_sweep {
        bump("path_sweep", 35);
    }

    let threshold = policy_threshold(policy);
    let is_bot = score >= threshold;
    let bot_reason = if is_bot {
        primary_reason.map(|(reason, _)| reason.to_string())
    } else {
        None
    };

    BotClassification {
        is_bot,
        bot_score: score,
        bot_reason,
    }
}
