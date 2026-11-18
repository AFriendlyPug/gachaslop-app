//! Banner lists for the built-in games, kept on disk and only fetched again when the stored data runs
//! out, plus their character icons (downloaded once, deleted once no stored banner needs them).
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{BACKOFF, BA_BANNERS_URL, BA_ICON_URL, CHECK_DELAY, MANUAL_REFRESH_GAP, ZZZ_CALENDAR_URL,
                    ZZZ_CALENDAR_UTC_OFFSET, ZZZ_EU_UTC_OFFSET};
use crate::net;
use crate::time::{parse_iso, Ts};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedKind {
    Ba,
    Zzz,
}

impl FeedKind {
    pub const ALL: [FeedKind; 2] = [FeedKind::Ba, FeedKind::Zzz];

    pub fn key(self) -> &'static str {
        match self {
            FeedKind::Ba => "ba",
            FeedKind::Zzz => "zzz",
        }
    }

    pub fn from_key(key: &str) -> Option<FeedKind> {
        FeedKind::ALL.into_iter().find(|k| k.key() == key)
    }

    pub fn name(self) -> &'static str {
        match self {
            FeedKind::Ba => "Blue Archive",
            FeedKind::Zzz => "Zenless Zone Zero",
        }
    }

    fn url(self) -> &'static str {
        match self {
            FeedKind::Ba => BA_BANNERS_URL,
            FeedKind::Zzz => ZZZ_CALENDAR_URL,
        }
    }

    pub fn credit(self) -> &'static str {
        match self {
            FeedKind::Ba => "Data: ba.joexyz.online · Icons: SchaleDB (both unofficial)",
            FeedKind::Zzz => "Data: HoYoLAB via api.ennead.cc (unofficial)",
        }
    }

    /// Whether listed future banners are only projections (BA's are projected from the JP schedule).
    pub fn upcoming_projected(self) -> bool {
        self == FeedKind::Ba
    }

    pub fn parse(self, payload: &Value) -> Result<Vec<Rotation>, String> {
        match self {
            FeedKind::Ba => parse_ba(payload),
            FeedKind::Zzz => parse_zzz(payload),
        }
    }
}

/// A character's icon: where it's kept (relative to the icons folder) and where to download it from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Icon {
    pub file: String,
    pub url: String,
}

/// A featured character or W-Engine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pick {
    pub name: String,
    pub marks: String, // e.g. "🌟 🎁" ('' for none)
    pub icon: Option<Icon>,
    /// A ZZZ agent's signature W-Engine, on the same phase's W-Engine banner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<Box<Pick>>,
}

/// One banner period: every pickup that shares the same start and end (UTC).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rotation {
    pub key: String,
    pub start: Ts,
    pub end: Ts,
    pub picks: Vec<Pick>,    // the featured characters (with their W-Engines, for ZZZ)
    pub extras: Vec<Pick>,   // featured W-Engines no agent was paired with
    pub details: Vec<String>,
}

impl Rotation {
    pub(crate) fn new(kind: FeedKind, start: Ts, end: Ts, picks: Vec<Pick>, extras: Vec<Pick>,
                      details: Vec<String>) -> Rotation {
        Rotation { key: format!("{}|{start}|{end}", kind.key()), start, end, picks, extras, details }
    }

    pub fn is_live(&self, now: Ts) -> bool {
        self.start <= now && now < self.end
    }

    pub fn label(&self) -> String {
        self.picks.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
    }

    pub fn icons(&self) -> impl Iterator<Item = &Icon> {
        self.picks.iter().chain(&self.extras)
            .flat_map(|p| p.icon.iter().chain(p.engine.as_ref().and_then(|e| e.icon.as_ref())))
    }
}

/// One feed's stored banners and when to check it next.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FeedState {
    pub rotations: Vec<Rotation>,
    pub fetched_at: Option<Ts>, // last successful fetch
    pub checked_at: Option<Ts>, // last attempt, successful or not
    pub next_check: Option<Ts>, // None = as soon as possible (nothing stored yet)
    pub failures: usize,        // checks in a row that failed or found nothing new
    pub last_error: Option<String>,
}

impl FeedState {
    pub fn is_due(&self, now: Ts) -> bool {
        self.next_check.is_none_or(|at| at <= now)
    }

    /// A manual refresh makes no request if the last successful fetch was this recent.
    pub fn fresh(&self, now: Ts) -> bool {
        self.fetched_at.is_some_and(|at| now - at < MANUAL_REFRESH_GAP)
    }

    /// Whether a fetch made after this rotation started still listed it, so it's not just a projection.
    pub fn confirmed(&self, rot: &Rotation) -> bool {
        self.fetched_at.is_some_and(|at| at >= rot.start)
    }

    /// Drop banners that have ended. True if any were dropped.
    pub fn drop_ended(&mut self, now: Ts) -> bool {
        let before = self.rotations.len();
        self.rotations.retain(|r| r.end > now);
        self.rotations.len() != before
    }

    /// Record a check's result and schedule the next one. On failure the old banners are kept.
    pub fn record(&mut self, kind: FeedKind, result: Result<Vec<Rotation>, String>, now: Ts) {
        self.checked_at = Some(now);
        let ok = match result {
            Ok(rotations) => {
                (self.rotations, self.fetched_at, self.last_error) = (rotations, Some(now), None);
                true
            }
            Err(err) => {
                self.last_error = Some(err);
                false
            }
        };
        self.drop_ended(now);
        match data_runs_out(kind, &self.rotations, now) {
            Some(at) if ok => {
                (self.next_check, self.failures) = (Some(at + CHECK_DELAY), 0);
            }
            _ => {
                // failed, or the source hasn't caught up yet: try again later, waiting longer each time
                self.next_check = Some(now + BACKOFF[self.failures.min(BACKOFF.len() - 1)]);
                self.failures += 1;
            }
        }
    }
}

/// When the stored banners stop being enough: the next banner change the stored data can't vouch for.
/// None if nothing live or upcoming is stored.
pub fn data_runs_out(kind: FeedKind, rotations: &[Rotation], now: Ts) -> Option<Ts> {
    let next_start = rotations.iter().filter(|r| r.start > now).map(|r| r.start).min();
    let first_end = rotations.iter().filter(|r| r.is_live(now)).map(|r| r.end).min();
    if kind.upcoming_projected() {
        next_start.or(first_end) // confirm each projected banner when it's due to start
    } else {
        first_end.or(next_start) // the feed only lists what's on now
    }
}

/// Both feeds, as saved in banners.json.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Feeds {
    pub ba: FeedState,
    pub zzz: FeedState,
}

impl Feeds {
    pub fn get(&self, kind: FeedKind) -> &FeedState {
        match kind {
            FeedKind::Ba => &self.ba,
            FeedKind::Zzz => &self.zzz,
        }
    }

    pub fn get_mut(&mut self, kind: FeedKind) -> &mut FeedState {
        match kind {
            FeedKind::Ba => &mut self.ba,
            FeedKind::Zzz => &mut self.zzz,
        }
    }

    /// Icon files some stored live or upcoming banner still uses.
    pub fn wanted_icons(&self) -> HashSet<String> {
        FeedKind::ALL.iter().flat_map(|k| &self.get(*k).rotations).flat_map(|r| r.icons())
            .map(|i| i.file.clone()).collect()
    }
}

// ---- parsing ---------------------------------------------------------------------------------------
fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        _ => false,
    }
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value[field].as_str().ok_or_else(|| format!("missing {field}"))
}

fn time(value: &Value, field: &str) -> Result<Ts, String> {
    let raw = text(value, field)?;
    parse_iso(raw).ok_or_else(|| format!("bad date {raw:?}"))
}

fn ba_icon(student_id: u64) -> Icon {
    Icon { file: format!("ba/{student_id}.webp"), url: BA_ICON_URL.replace("{id}", &student_id.to_string()) }
}

/// The API lists one pickup student per banner: group them into rotations by start and end.
pub fn parse_ba(payload: &Value) -> Result<Vec<Rotation>, String> {
    let banners = payload["data"].as_array().ok_or("missing data list")?;
    let mut groups: BTreeMap<(Ts, Ts), Vec<Pick>> = BTreeMap::new();
    for banner in banners {
        let span = (time(banner, "startDate")?, time(banner, "endDate")?);
        let students = banner["pickupStudents"].as_array().ok_or("missing pickupStudents")?;
        for student in students {
            let mut marks = Vec::new();
            if truthy(&student["isFestGlobal"]) {
                marks.push("🌟");
            } else if truthy(&student["isLimitedGlobal"]) {
                marks.push("⏳");
            }
            if truthy(&banner["freePulls"]) {
                marks.push("🎁");
            }
            groups.entry(span).or_default().push(Pick {
                name: text(student, "name")?.to_string(),
                marks: marks.join(" "),
                icon: student["schaleDbId"].as_u64().map(ba_icon),
                engine: None,
            });
        }
    }
    Ok(groups.into_iter().map(|((start, end), picks)| Rotation::new(FeedKind::Ba, start, end, picks, vec![], vec![]))
        .collect())
}

/// A pick's marks in words: "🌟 🎁" is ["Fest", "Free pulls"]. The popup shows them as tags.
pub fn mark_words(marks: &str) -> Vec<&'static str> {
    marks.split_whitespace().filter_map(|m| match m {
        "🌟" => Some("Fest"),
        "⏳" => Some("Limited"),
        "🎁" => Some("Free pulls"),
        _ => None,
    }).collect()
}

fn zzz_pick(kind: &str, item: &Value) -> Option<Pick> {
    let name = item["name"].as_str()?.to_string();
    let icon = item["icon"].as_str().filter(|u| !u.is_empty()).map(|url| {
        let path = url.split(['?', '#']).next().unwrap_or(url);
        let suffix = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("png");
        Icon { file: format!("zzz/{kind}-{}.{suffix}", item["id"]), url: url.to_string() }
    });
    Some(Pick { name, marks: String::new(), icon, engine: None })
}

/// Give each agent its signature W-Engine: the featured one of the same specialty (the API's
/// "profession"; a signature W-Engine always shares its agent's), in list order when two agents share one.
/// Takes (pick, specialty) pairs; returns the agents and any W-Engines left over.
pub fn pair_engines(agents: Vec<(Pick, String)>, engines: Vec<(Pick, String)>) -> (Vec<Pick>, Vec<Pick>) {
    let mut left: Vec<Option<(Pick, String)>> = engines.into_iter().map(Some).collect();
    let agents = agents.into_iter().map(|(mut agent, role)| {
        let same = left.iter_mut().find(|e| e.as_ref().is_some_and(|(_, r)| !role.is_empty() && *r == role));
        agent.engine = same.and_then(Option::take).map(|(engine, _)| Box::new(engine));
        agent
    }).collect();
    (agents, left.into_iter().flatten().map(|(engine, _)| engine).collect())
}

/// The calendar has one entry per banner type (agents, W-Engines) in each phase: group them by start and
/// end. A rotation's picks are its S-rank agents, each with its S-rank W-Engine (or the S-rank W-Engines if
/// it has no agents); A-rank agents go in details.
pub fn parse_zzz(payload: &Value) -> Result<Vec<Rotation>, String> {
    let banners = payload["banners"].as_array().ok_or("missing banners list")?;
    let shift = ZZZ_CALENDAR_UTC_OFFSET - ZZZ_EU_UTC_OFFSET; // same local time on the EU server
    type Phase = (Vec<Value>, Vec<Value>); // its agents and its W-Engines
    let mut groups: BTreeMap<(Ts, Ts), Phase> = BTreeMap::new();
    for banner in banners {
        let times = (banner["start_time"].as_i64().filter(|t| *t > 0), banner["end_time"].as_i64().filter(|t| *t > 0));
        let (Some(start), Some(end)) = times else { continue };
        let (agents, engines) = groups.entry((start + shift, end + shift)).or_default();
        agents.extend(banner["agents"].as_array().cloned().unwrap_or_default());
        engines.extend(banner["w_engines"].as_array().cloned().unwrap_or_default());
    }
    let is_s = |v: &&Value| v["rarity"].as_str() == Some("S");
    let top = |list: &[Value], kind| -> Vec<(Pick, String)> {
        list.iter().filter(is_s)
            .filter_map(|v| Some((zzz_pick(kind, v)?, v["profession"].as_str().unwrap_or_default().to_string())))
            .collect()
    };
    let mut rotations = Vec::new();
    for ((start, end), (agents, engines)) in groups {
        let (top_agents, top_engines) = (top(&agents, "agent"), top(&engines, "wengine"));
        let (picks, extras) = if top_agents.is_empty() {
            (top_engines.into_iter().map(|(engine, _)| engine).collect(), vec![])
        } else {
            pair_engines(top_agents, top_engines)
        };
        if picks.is_empty() {
            continue;
        }
        let others: Vec<_> = agents.iter().filter(|a| !is_s(a)).filter_map(|a| a["name"].as_str()).collect();
        let details = if others.is_empty() { vec![] } else { vec![format!("A-rank agents: {}", others.join(", "))] };
        rotations.push(Rotation::new(FeedKind::Zzz, start, end, picks, extras, details));
    }
    Ok(rotations)
}

// ---- network ---------------------------------------------------------------------------------------
pub fn fetch(kind: FeedKind) -> Result<Vec<Rotation>, String> {
    let body = net::get(kind.url())?;
    let payload: Value = serde_json::from_slice(&body).map_err(|e| format!("not JSON ({e})"))?;
    kind.parse(&payload)
}

// ---- icons -----------------------------------------------------------------------------------------
const IMAGE_SIGNATURES: [&[u8]; 2] = [b"\x89PNG", b"\xff\xd8\xff"]; // PNG, JPEG (WebP checked separately)

pub fn is_image(data: &[u8]) -> bool {
    IMAGE_SIGNATURES.iter().any(|sig| data.starts_with(sig))
        || (data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP")
}

/// "png", "jpg" or "webp" for an image (see is_image).
pub fn image_ext(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG") { "png" } else if data.starts_with(b"\xff\xd8") { "jpg" } else { "webp" }
}

/// Write a file via a temporary file, so a crash never leaves half of it behind.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, path)
}

/// Icons stored banners use that aren't on disk yet.
pub fn missing_icons(feeds: &Feeds, dir: &Path) -> Vec<Icon> {
    let mut seen = HashSet::new();
    FeedKind::ALL.iter().flat_map(|k| &feeds.get(*k).rotations).flat_map(|r| r.icons())
        .filter(|i| seen.insert(i.file.clone()) && !dir.join(&i.file).exists()).cloned().collect()
}

/// Download one icon into the icons folder. Not saved if the response isn't an image (e.g. an error page).
pub fn download_icon(dir: &Path, icon: &Icon) -> Result<(), String> {
    let data = net::get(&icon.url)?;
    if !is_image(&data) {
        return Err(format!("{} is not an image", icon.file));
    }
    write_atomic(&dir.join(&icon.file), &data).map_err(|e| e.to_string())
}

/// Delete every file in the icons folder that no stored banner uses, and any leftover temporary files.
pub fn prune_icons(dir: &Path, keep: &HashSet<String>) -> usize {
    fn walk(dir: &Path, root: &Path, keep: &HashSet<String>, removed: &mut usize) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, keep, removed);
                continue;
            }
            let rel = path.strip_prefix(root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            if !keep.contains(&rel) && std::fs::remove_file(&path).is_ok() {
                *removed += 1;
            }
        }
    }
    let mut removed = 0;
    walk(dir, dir, keep, &mut removed);
    removed
}
