use actix_web::{
    get, http::header::{
        Accept, CacheControl, CacheDirective, ContentType, EntityTag, Header, IfNoneMatch,
        CACHE_CONTROL, CONTENT_TYPE, ETAG, LOCATION,
    }, web::{self}, App, HttpRequest,
    HttpResponse,
    HttpServer,
};
use cached::{Cached, Return};
use csscolorparser::parse;
use rsbadges::{Badge, Style};
use std::collections::HashSet;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;
use tempfile::TempDir;
use tokei::{Language, LanguageType, Languages};

const BLUE: &str = "#007ec6";
const GREY: &str = "#555555";

const BLANKS: &str = "blank lines";
const CODE: &str = "lines of code";
const COMMENTS: &str = "comments";
const FILES: &str = "files";
const LINES: &str = "total lines";

const HASH_LENGTH: usize = 40;

const THOUSAND: usize = 1_000;
const MILLION: usize = 1_000_000;
const BILLION: usize = 1_000_000_000;

const SHA_CACHE_DURATION: Duration = Duration::from_secs(300);
const STATISTICS_CACHE_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

static CONTENT_TYPE_SVG: LazyLock<ContentType> =
    LazyLock::new(|| ContentType("image/svg+xml".parse().unwrap()));

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::init();

    HttpServer::new(|| {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .service(redirect_index)
            .service(create_badge)
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}

#[get("/")]
async fn redirect_index() -> HttpResponse {
    HttpResponse::PermanentRedirect()
        .insert_header((LOCATION, "https://github.com/Zitronenjoghurt/lemon-tokei"))
        .finish()
}

macro_rules! respond {
    ($status:ident) => {{ HttpResponse::$status().finish() }};

    ($status:ident, $body:expr) => {{
        HttpResponse::$status()
            .set(CONTENT_TYPE_SVG.clone())
            .body($body)
    }};

    ($status:ident, $accept:expr, $body:expr, $etag:expr) => {{
        HttpResponse::$status()
            .insert_header((CACHE_CONTROL, CacheControl(vec![CacheDirective::NoCache])))
            .insert_header((ETAG, EntityTag::new(false, $etag)))
            .insert_header((
                CONTENT_TYPE,
                if $accept == ContentType::json() {
                    ContentType::json()
                } else {
                    CONTENT_TYPE_SVG.clone()
                },
            ))
            .body($body)
    }};
}

#[allow(non_snake_case)]
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BadgeQuery {
    category: Option<String>,
    label: Option<String>,
    style: Option<String>,
    color: Option<String>,
    logo: Option<String>,
    r#type: Option<String>,
    show_language: Option<String>,
    language_rank: Option<String>,
    branch: Option<String>,
}

#[get("/b1/{domain}/{user}/{repo}")]
async fn create_badge(
    request: HttpRequest,
    path: web::Path<(String, String, String)>,
    web::Query(query): web::Query<BadgeQuery>,
) -> actix_web::Result<HttpResponse> {
    let (domain, user, repo) = path.into_inner();
    let category = query.category.unwrap_or_else(|| "lines".to_owned());
    let (label, no_label) = match query.label {
        Some(v) => (v, false),
        None => ("".to_owned(), true),
    };
    let style: String = query.style.unwrap_or_else(|| "plastic".to_owned());
    let color: String = query.color.unwrap_or_else(|| BLUE.to_owned());
    let logo: String = query.logo.unwrap_or_else(|| "".to_owned());
    let r#type: String = query.r#type.unwrap_or_else(|| "".to_owned());
    let show_language: bool = query
        .show_language
        .unwrap_or_else(|| "".to_owned())
        .parse::<bool>()
        .unwrap_or(false);
    let language_rank: usize = match query.language_rank {
        Some(s) => s.parse::<usize>().unwrap_or(0),
        None => 1,
    };
    let branch: String = query.branch.unwrap_or_else(|| "".to_owned());

    let content_type: ContentType = if let Ok(accept) = Accept::parse(&request) {
        if accept == Accept::json() {
            ContentType::json()
        } else {
            CONTENT_TYPE_SVG.clone()
        }
    } else {
        CONTENT_TYPE_SVG.clone()
    };

    let mut domain = percent_encoding::percent_decode_str(&domain).decode_utf8()?;

    // For backwards compatibility if a domain isn't specified we append `.com`.
    if !domain.contains('.') {
        domain += ".com";
    }

    let url: &str = &format!("https://{}/{}/{}", domain, user, repo);

    let sha_and_branch =
        resolve_sha(&domain, &user, &repo, &branch).map_err(actix_web::error::ErrorBadRequest)?;

    let (sha, branch_name) = sha_and_branch
        .split_once('#')
        .ok_or_else(|| actix_web::error::ErrorBadRequest(eyre::eyre!("Invalid SHA resolve")))?;

    if let Ok(if_none_match) = IfNoneMatch::parse(&request) {
        log::debug!("Checking If-None-Match: {}#{}", sha, branch_name);
        let entity_tag: EntityTag = EntityTag::new(false, etag_identifier(sha, branch_name));
        let found_match: bool = match if_none_match {
            IfNoneMatch::Any => false,
            IfNoneMatch::Items(items) => items
                .iter()
                .any(|etag: &EntityTag| etag.weak_eq(&entity_tag)),
        };

        if found_match {
            CACHE
                .lock()
                .cache_get(&repo_identifier(url, sha, branch_name));
            log::info!("{}#{}#{} Not Modified", url, sha, branch_name);
            return Ok(respond!(NotModified));
        }
    }

    let entry: Return<Vec<(LanguageType, Language)>> =
        get_statistics(url, sha, branch_name).map_err(actix_web::error::ErrorBadRequest)?;

    if entry.was_cached {
        log::info!("{}#{}#{} Cache hit", url, sha, branch_name);
    }

    let language_types: HashSet<LanguageType> = r#type
        .split(',')
        .filter_map(|s: &str| str::parse::<LanguageType>(s).ok())
        .collect::<HashSet<LanguageType>>();

    let languages: Vec<(LanguageType, Language)> = if language_types.is_empty() {
        entry.value
    } else {
        entry
            .value
            .into_iter()
            .filter(|(language_type, _)| language_types.contains(language_type))
            .collect()
    };
    let ranking_language = if !show_language {
        String::new()
    } else if languages.is_empty() {
        "No Languages".to_owned()
    } else if language_rank == 0 || language_rank > languages.len() {
        "N/A".to_owned()
    } else {
        let (ranking_language_type, _) = languages[language_rank - 1];
        ranking_language_type.name().to_owned()
    };

    let mut stats = Language::new();
    for (_, language) in &languages {
        stats += language.clone();
    }

    log::info!(
        "{url}#{sha}#{branch_name} - Languages (most common to least common) {languages:#?} Lines {lines} Code {code} Comments {comments} Blanks {blanks}",
        url = url,
        sha = sha,
        branch_name = branch_name,
        languages = languages,
        lines = stats.lines(),
        code = stats.code,
        comments = stats.comments,
        blanks = stats.blanks
    );

    let badge: String = make_badge(
        &content_type,
        &stats,
        &category,
        &label,
        &style,
        &color,
        &logo,
        &ranking_language,
        no_label,
    )
    .await?;

    Ok(respond!(
        Ok,
        content_type,
        badge,
        etag_identifier(sha, branch_name)
    ))
}

#[cached::proc_macro::cached(
    name = "SHA_CACHE",
    result = true,
    ty = "cached::TimedSizedCache<String, String>",
    create = "{ cached::TimedSizedCache::with_size_and_lifespan(1000, SHA_CACHE_DURATION) }",
    convert = r#"{ format!("{}#{}#{}#{}", domain, user, repo, branch_name_override) }"#
)]
fn resolve_sha(
    domain: &str,
    user: &str,
    repo: &str,
    branch_name_override: &str,
) -> eyre::Result<String> {
    let url = format!("https://{}/{}/{}", domain, user, repo);

    let ls_remote = Command::new("git")
        .args(["ls-remote", "--symref", &url, "HEAD", "refs/heads/**"])
        .output()?;

    let ls_remote_output =
        String::from_utf8(ls_remote.stdout).map_err(|_| eyre::eyre!("Invalid ls-remote output"))?;

    eyre::ensure!(!ls_remote_output.is_empty(), "Empty ls-remote output");

    let git_lines: Vec<&str> = ls_remote_output.split('\n').collect();
    eyre::ensure!(git_lines.len() > 1, "Not enough ls-remote lines");

    let mut iter = git_lines.iter();

    // First line: resolve HEAD branch
    let head_branch = iter
        .next()
        .and_then(|s| s.strip_prefix("ref: refs/heads/"))
        .and_then(|s| s.strip_suffix("\tHEAD"))
        .unwrap_or("");

    iter.next(); // skip HEAD sha line

    let branch_name = if branch_name_override.is_empty() {
        head_branch
    } else {
        branch_name_override
    };

    for &line in iter {
        if let Some((s, bn)) = line.split_once("\trefs/heads/")
            && bn == branch_name
        {
            eyre::ensure!(s.len() == HASH_LENGTH, "Invalid SHA length");
            // Return "sha#resolved_branch" so caller has both
            return Ok(format!("{}#{}", s, branch_name));
        }
    }

    eyre::bail!("Branch '{}' not found", branch_name)
}

#[cached::proc_macro::cached(
    name = "CACHE",
    result = true,
    with_cached_flag = true,
    ty = "cached::TimedSizedCache<String, cached::Return<Vec<(LanguageType,Language)>>>",
    create = "{ cached::TimedSizedCache::with_size_and_lifespan(1000, STATISTICS_CACHE_DURATION) }",
    convert = r#"{ repo_identifier(url, _sha, branch_name) }"#
)]
fn get_statistics(
    url: &str,
    _sha: &str,
    branch_name: &str,
) -> eyre::Result<Return<Vec<(LanguageType, Language)>>> {
    log::info!("{} - Cloning", url);
    let temp_dir: TempDir = TempDir::new()?;
    let temp_path: &str = temp_dir.path().to_str().unwrap();

    Command::new("git")
        .args([
            "clone",
            url,
            temp_path,
            "--depth",
            "1",
            "--branch",
            branch_name,
        ])
        .output()?;

    let mut languages: Languages = Languages::new();
    log::info!("{} - Getting Statistics", url);
    languages.get_statistics(&[temp_path], &[], &tokei::Config::default());

    for (_, language) in languages.iter_mut() {
        for report in &mut language.reports {
            report.name = report.name.strip_prefix(temp_path)?.to_owned();
        }
        for child in &mut language.children.values_mut() {
            for language in child.iter_mut() {
                language.name = language.name.strip_prefix(temp_path)?.to_owned();
            }
        }
    }

    let mut languages_sorted_by_lines_of_code: Vec<(LanguageType, Language)> =
        languages.into_iter().collect();
    languages_sorted_by_lines_of_code.sort_by_key(|(_, b)| std::cmp::Reverse(b.code));

    Ok(Return::new(languages_sorted_by_lines_of_code))
}

fn repo_identifier(url: &str, sha: &str, branch_name: &str) -> String {
    format!("{}#{}#{}", url, sha, branch_name)
}

fn etag_identifier(sha: &str, branch_name: &str) -> String {
    format!("{}#{}", sha, branch_name)
}

#[allow(clippy::too_many_arguments)]
async fn make_badge(
    content_type: &ContentType,
    stats: &Language,
    category: &str,
    label: &str,
    style: &str,
    color: &str,
    logo: &str,
    ranking_language: &str,
    no_label: bool,
) -> actix_web::Result<String> {
    if *content_type == ContentType::json() {
        return Ok(serde_json::to_string(&stats)?);
    }

    if !ranking_language.is_empty() {
        return make_badge_style(label, ranking_language, color, style, logo).await;
    }

    let (amount, label) = match category {
        "code" => (stats.code, if no_label { CODE } else { label }),
        "files" => (stats.reports.len(), if no_label { FILES } else { label }),
        "blanks" => (stats.blanks, if no_label { BLANKS } else { label }),
        "comments" => (stats.comments, if no_label { COMMENTS } else { label }),
        _ => (stats.lines(), if no_label { LINES } else { label }),
    };

    let amount: String = if amount >= BILLION {
        format!("{:.1}B", trim_and_float(amount, BILLION))
    } else if amount >= MILLION {
        format!("{:.1}M", trim_and_float(amount, MILLION))
    } else if amount >= THOUSAND {
        format!("{:.1}K", trim_and_float(amount, THOUSAND))
    } else {
        amount.to_string()
    };

    make_badge_style(label, &amount, color, style, logo).await
}

async fn make_badge_style(
    label: &str,
    msg: &str,
    color: &str,
    style: &str,
    logo: &str,
) -> Result<String, actix_web::Error> {
    fn badge(label: &str, msg: &str, color: &str) -> Badge {
        Badge {
            label_text: label.to_owned(),
            label_color: GREY.to_owned(),
            msg_text: msg.to_owned(),
            msg_color: match parse(color) {
                Ok(result) => result.to_css_hex(),
                Err(_error) => BLUE.to_owned(),
            },
            ..Badge::default()
        }
    }

    let badge_with_logo: Badge = Badge {
        logo: logo.to_owned(),
        embed_logo: !logo.is_empty(),
        ..badge(label, msg, color)
    };

    fn stylize_badge(badge: Badge, style: &str) -> Style {
        match style {
            "flat" => Style::Flat(badge),
            "flat-square" => Style::FlatSquare(badge),
            "plastic" => Style::Plastic(badge),
            "for-the-badge" => Style::ForTheBadge(badge),
            "social" => Style::Social(badge),
            _ => Style::Flat(badge),
        }
    }

    match stylize_badge(badge_with_logo, style).generate_svg() {
        Ok(s) => Ok(s),
        Err(_e) => Ok(stylize_badge(badge(label, msg, color), style)
            .generate_svg()
            .unwrap()),
    }
}

fn trim_and_float(num: usize, trim: usize) -> f64 {
    (num as f64) / (trim as f64)
}
