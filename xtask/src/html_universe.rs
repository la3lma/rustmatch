//! Render the repository's human-scale Markdown as a navigable HTML mirror.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const OUTPUT_ROOT: &str = "docs/html-universe";
const INDEX: &str = "docs/html-universe/index.html";
const REPOSITORY_ROOT: &str = "docs/html-universe/repository";
const MERMAID_ROOT: &str = "docs/html-universe/assets/mermaid";
const MERMAID_CLI_PACKAGE: &str = "@mermaid-js/mermaid-cli@11.16.0";
const MERMAID_SOURCE_MARKER: &str = "rustmatch-mermaid-source-sha256";
const LEDGER_HTML: &str = "docs/optimization-attempt-ledger.html";
const AUDIT_SCALE_SOURCE: &str = "docs/optimization-and-scale.md";

struct GeneratedFile {
    path: PathBuf,
    contents: Vec<u8>,
}

struct MermaidBlock {
    range: Range<usize>,
    source: String,
    title: String,
}

struct MermaidDiagram {
    hash: String,
    source: String,
    path: PathBuf,
}

pub fn render() -> Result<(), String> {
    let generated = generate(true)?;
    let root = Path::new(OUTPUT_ROOT);
    if repository_path(root).exists() {
        fs::remove_dir_all(repository_path(root))
            .map_err(|error| format!("could not clear {OUTPUT_ROOT}: {error}"))?;
    }
    for file in generated {
        if let Some(parent) = file.path.parent() {
            fs::create_dir_all(repository_path(parent))
                .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
        }
        fs::write(repository_path(&file.path), file.contents)
            .map_err(|error| format!("could not write {}: {error}", file.path.display()))?;
    }
    println!("rendered HTML evidence universe at {INDEX}");
    Ok(())
}

pub fn verify() -> Result<(), String> {
    let generated = generate(false)?;
    let expected_paths = generated
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    let mut actual_paths = Vec::new();
    collect_files(Path::new(OUTPUT_ROOT), &mut actual_paths)?;
    let actual_paths = actual_paths.into_iter().collect::<BTreeSet<_>>();
    if actual_paths != expected_paths {
        let missing = expected_paths
            .difference(&actual_paths)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        let extra = actual_paths
            .difference(&expected_paths)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>();
        return Err(format!(
            "HTML evidence universe file set is stale; missing={missing:?} extra={extra:?}"
        ));
    }
    for file in generated {
        let actual = fs::read(repository_path(&file.path))
            .map_err(|error| format!("could not read {}: {error}", file.path.display()))?;
        if actual != file.contents {
            return Err(format!(
                "{} is stale; run `cargo xtask optimization-ledger`",
                file.path.display()
            ));
        }
    }
    println!("HTML evidence universe is current");
    Ok(())
}

pub fn ledger_evidence_href(value: &str) -> String {
    let Some((path_part, suffix)) = split_local_url(value) else {
        return value.to_owned();
    };
    let Ok(source) = normalize_path(&Path::new("docs").join(path_part)) else {
        return value.to_owned();
    };
    if source.extension().and_then(|extension| extension.to_str()) != Some("md")
        || !should_mirror(&source)
        || !repository_path(&source).is_file()
    {
        return value.to_owned();
    }
    format!(
        "{}{suffix}",
        relative_url(Path::new("docs"), &mirror_path(&source))
    )
}

fn generate(render_diagrams: bool) -> Result<Vec<GeneratedFile>, String> {
    let sources = markdown_sources()?;
    let source_set = sources.iter().cloned().collect::<BTreeSet<_>>();
    let diagrams = mermaid_diagrams(&sources)?;
    let mut generated = if render_diagrams {
        render_mermaid_assets(&diagrams)?
    } else {
        load_mermaid_assets(&diagrams)?
    };
    generated.reserve(sources.len() + 1);
    for source in &sources {
        generated.push(GeneratedFile {
            path: mirror_path(source),
            contents: render_document(source, &source_set)?.into_bytes(),
        });
    }
    generated.push(GeneratedFile {
        path: PathBuf::from(INDEX),
        contents: render_index(&sources)?.into_bytes(),
    });
    generated.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(generated)
}

fn mermaid_diagrams(sources: &[PathBuf]) -> Result<Vec<MermaidDiagram>, String> {
    let mut diagrams = BTreeMap::<String, MermaidDiagram>::new();
    for source in sources {
        let markdown = fs::read_to_string(repository_path(source))
            .map_err(|error| format!("could not read {}: {error}", source.display()))?;
        for block in mermaid_blocks(&markdown)? {
            let hash = mermaid_hash(&block.source);
            let diagram = MermaidDiagram {
                path: mermaid_asset_path(&hash),
                hash: hash.clone(),
                source: block.source,
            };
            if let Some(existing) = diagrams.get(&hash) {
                if existing.source != diagram.source {
                    return Err(format!(
                        "SHA-256 collision between Mermaid diagrams in {}",
                        source.display()
                    ));
                }
            } else {
                diagrams.insert(hash, diagram);
            }
        }
    }
    Ok(diagrams.into_values().collect())
}

fn render_mermaid_assets(diagrams: &[MermaidDiagram]) -> Result<Vec<GeneratedFile>, String> {
    if diagrams.is_empty() {
        return Ok(Vec::new());
    }
    let temporary = std::env::temp_dir().join(format!(
        "rustmatch-html-universe-mermaid-{}",
        std::process::id()
    ));
    if temporary.exists() {
        fs::remove_dir_all(&temporary)
            .map_err(|error| format!("could not clear {}: {error}", temporary.display()))?;
    }
    fs::create_dir_all(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;

    let result = diagrams
        .iter()
        .enumerate()
        .map(|(index, diagram)| {
            let input = temporary.join(format!("{index}.mmd"));
            let output = temporary.join(format!("{index}.svg"));
            fs::write(&input, &diagram.source)
                .map_err(|error| format!("could not write {}: {error}", input.display()))?;
            let command_output = Command::new("npx")
                .arg("--yes")
                .arg(MERMAID_CLI_PACKAGE)
                .arg("-i")
                .arg(&input)
                .arg("-o")
                .arg(&output)
                .arg("-b")
                .arg("transparent")
                .output()
                .map_err(|error| format!("could not start Mermaid renderer: {error}"))?;
            if !command_output.status.success() {
                return Err(format!(
                    "Mermaid renderer failed for {}: {}",
                    diagram.path.display(),
                    String::from_utf8_lossy(&command_output.stderr).trim()
                ));
            }
            let svg = fs::read_to_string(&output)
                .map_err(|error| format!("could not read {}: {error}", output.display()))?;
            Ok(GeneratedFile {
                path: diagram.path.clone(),
                contents: bind_svg_to_source(svg, &diagram.hash)?.into_bytes(),
            })
        })
        .collect::<Result<Vec<_>, String>>();
    let cleanup = fs::remove_dir_all(&temporary)
        .map_err(|error| format!("could not remove {}: {error}", temporary.display()));
    match (result, cleanup) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(files), Ok(())) => Ok(files),
    }
}

fn load_mermaid_assets(diagrams: &[MermaidDiagram]) -> Result<Vec<GeneratedFile>, String> {
    diagrams
        .iter()
        .map(|diagram| {
            let contents = fs::read(repository_path(&diagram.path)).map_err(|error| {
                format!(
                    "{} is missing or unreadable ({error}); run `cargo xtask optimization-ledger`",
                    diagram.path.display()
                )
            })?;
            verify_svg_source_binding(&contents, &diagram.hash, &diagram.path)?;
            Ok(GeneratedFile {
                path: diagram.path.clone(),
                contents,
            })
        })
        .collect()
}

fn bind_svg_to_source(mut svg: String, hash: &str) -> Result<String, String> {
    let svg_start = svg
        .find("<svg")
        .ok_or_else(|| "Mermaid renderer returned output without an SVG root".to_owned())?;
    let root_end = svg[svg_start..]
        .find('>')
        .map(|offset| svg_start + offset + 1)
        .ok_or_else(|| "Mermaid renderer returned a malformed SVG root".to_owned())?;
    svg.insert_str(root_end, &svg_source_marker(hash));
    Ok(svg)
}

fn verify_svg_source_binding(contents: &[u8], hash: &str, path: &Path) -> Result<(), String> {
    let svg = std::str::from_utf8(contents)
        .map_err(|error| format!("{} is not UTF-8 SVG: {error}", path.display()))?;
    if svg.contains(&svg_source_marker(hash)) {
        Ok(())
    } else {
        Err(format!(
            "{} is not bound to its current Mermaid source; run `cargo xtask optimization-ledger`",
            path.display()
        ))
    }
}

fn svg_source_marker(hash: &str) -> String {
    format!("<metadata id=\"{MERMAID_SOURCE_MARKER}\">{hash}</metadata>")
}

fn mermaid_hash(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    let mut hash = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(hash, "{byte:02x}").expect("write to string");
    }
    hash
}

fn mermaid_asset_path(hash: &str) -> PathBuf {
    Path::new(MERMAID_ROOT).join(format!("{hash}.svg"))
}

fn markdown_sources() -> Result<Vec<PathBuf>, String> {
    let mut sources = Vec::new();
    collect_markdown(Path::new("."), &mut sources)?;
    sources.sort();
    Ok(sources)
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(repository_path(directory))
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("could not inspect {}: {error}", directory.display()))?;
        let path = normalize_path(&directory.join(entry.file_name()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            if !ignored_directory(&path) {
                collect_markdown(&path, output)?;
            }
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("md")
            && should_mirror(&path)
        {
            output.push(path);
        }
    }
    Ok(())
}

fn ignored_directory(path: &Path) -> bool {
    path == Path::new(OUTPUT_ROOT)
        || path.components().any(|component| {
            matches!(
                component,
                Component::Normal(name) if name == ".git" || name == "target"
            )
        })
}

fn should_mirror(path: &Path) -> bool {
    path != Path::new(AUDIT_SCALE_SOURCE)
}

fn mirror_path(source: &Path) -> PathBuf {
    let mut output = Path::new(REPOSITORY_ROOT).join(source);
    output.set_extension("html");
    output
}

fn mermaid_blocks(markdown: &str) -> Result<Vec<MermaidBlock>, String> {
    const START: &str = "```mermaid\n";
    const END: &str = "\n```";

    let mut blocks = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = markdown[cursor..].find(START) {
        let start = cursor + offset;
        if start > 0 && markdown.as_bytes()[start - 1] != b'\n' {
            cursor = start + START.len();
            continue;
        }
        let source_start = start + START.len();
        let source_end = markdown[source_start..]
            .find(END)
            .map(|offset| source_start + offset)
            .ok_or_else(|| "unterminated Mermaid code block".to_owned())?;
        let end = source_end + END.len();
        blocks.push(MermaidBlock {
            range: start..end,
            source: markdown[source_start..source_end].to_owned(),
            title: preceding_heading(&markdown[..start])
                .unwrap_or_else(|| "Mermaid diagram".to_owned()),
        });
        cursor = end;
    }
    Ok(blocks)
}

fn preceding_heading(markdown: &str) -> Option<String> {
    markdown.lines().rev().find_map(|line| {
        let heading = line.trim_start().strip_prefix('#')?.trim_start_matches('#');
        let heading = heading.trim();
        (!heading.is_empty()).then(|| heading.to_owned())
    })
}

fn render_mermaid_blocks(markdown: &str, output_parent: &Path) -> Result<String, String> {
    let blocks = mermaid_blocks(markdown)?;
    if blocks.is_empty() {
        return Ok(markdown.to_owned());
    }
    let mut rendered = String::with_capacity(markdown.len());
    let mut cursor = 0;
    for block in blocks {
        rendered.push_str(&markdown[cursor..block.range.start]);
        let hash = mermaid_hash(&block.source);
        let href = relative_url(output_parent, &mermaid_asset_path(&hash));
        write!(
            rendered,
            "<figure class=\"mermaid-diagram\"><a href=\"{}\" title=\"Open full-size SVG\">\
             <img src=\"{}\" alt=\"{} diagram\"></a>\
             <figcaption>{} · <a href=\"{}\">Open full-size SVG</a></figcaption></figure>",
            html_escape(&href),
            html_escape(&href),
            html_escape(&block.title),
            html_escape(&block.title),
            html_escape(&href)
        )
        .expect("write to string");
        cursor = block.range.end;
    }
    rendered.push_str(&markdown[cursor..]);
    Ok(rendered)
}

fn render_document(source: &Path, source_set: &BTreeSet<PathBuf>) -> Result<String, String> {
    let markdown = fs::read_to_string(repository_path(source))
        .map_err(|error| format!("could not read {}: {error}", source.display()))?;
    let output = mirror_path(source);
    let output_parent = output
        .parent()
        .ok_or_else(|| format!("{} has no parent", output.display()))?;
    let rendered_markdown = render_mermaid_blocks(&markdown, output_parent)?;
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM;
    let events = Parser::new_ext(&rendered_markdown, options)
        .map(|event| rewrite_event(event, source, output_parent, source_set));
    let mut body = String::new();
    html::push_html(&mut body, events);
    let title = document_title(&markdown, source);
    let index_href = relative_url(output_parent, Path::new(INDEX));
    let ledger_href = relative_url(output_parent, Path::new(LEDGER_HTML));
    let source_href = relative_url(output_parent, source);
    Ok(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title} · rustmatch evidence</title><style>{DOCUMENT_CSS}</style></head>\
         <body><nav><a class=\"brand\" href=\"{index_href}\">rustmatch evidence</a>\
         <div><a href=\"{ledger_href}\">Optimization ledger</a>\
         <a href=\"{index_href}\">HTML universe</a>\
         <a href=\"{source_href}\">Source Markdown</a></div></nav>\
         <header><div class=\"kicker\">Retained document</div><div class=\"path\">{path}</div></header>\
         <main>{body}</main><footer>Generated from <code>{path}</code> by \
         <code>cargo xtask optimization-ledger</code>.</footer>\
         <script>{HEADING_SCRIPT}</script></body></html>",
        title = html_escape(&title),
        path = html_escape(&path_text(source)),
    ))
}

fn rewrite_event<'a>(
    event: Event<'a>,
    source: &Path,
    output_parent: &Path,
    source_set: &BTreeSet<PathBuf>,
) -> Event<'a> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Link {
            link_type,
            dest_url: CowStr::from(rewrite_url(
                dest_url.as_ref(),
                source,
                output_parent,
                source_set,
            )),
            title,
            id,
        }),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => Event::Start(Tag::Image {
            link_type,
            dest_url: CowStr::from(rewrite_url(
                dest_url.as_ref(),
                source,
                output_parent,
                source_set,
            )),
            title,
            id,
        }),
        other => other,
    }
}

fn rewrite_url(
    value: &str,
    source: &Path,
    output_parent: &Path,
    source_set: &BTreeSet<PathBuf>,
) -> String {
    let Some((path_part, suffix)) = split_local_url(value) else {
        return value.to_owned();
    };
    let source_parent = source.parent().unwrap_or_else(|| Path::new("."));
    let Ok(target) = normalize_path(&source_parent.join(path_part)) else {
        return value.to_owned();
    };
    let destination = if source_set.contains(&target) {
        mirror_path(&target)
    } else if repository_path(&target).exists() {
        target
    } else {
        return value.to_owned();
    };
    format!("{}{suffix}", relative_url(output_parent, &destination))
}

fn split_local_url(value: &str) -> Option<(&str, &str)> {
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with('/')
        || value.starts_with("mailto:")
        || value.starts_with("data:")
        || value.contains("://")
    {
        return None;
    }
    let split = value
        .char_indices()
        .find_map(|(index, character)| matches!(character, '#' | '?').then_some(index))
        .unwrap_or(value.len());
    let (path, suffix) = value.split_at(split);
    (!path.is_empty()).then_some((path, suffix))
}

fn normalize_path(path: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => normalized.push(value),
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(format!("path escapes repository: {}", path.display()));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!(
                    "path must be repository-relative: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(normalized)
}

fn relative_url(from: &Path, to: &Path) -> String {
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let common = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in common..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[common..] {
        relative.push(component.as_os_str());
    }
    path_text(&relative)
}

fn document_title(markdown: &str, source: &Path) -> String {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map_or_else(
            || {
                source
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Document")
                    .to_owned()
            },
            ToOwned::to_owned,
        )
}

fn render_index(sources: &[PathBuf]) -> Result<String, String> {
    let index_parent = Path::new(INDEX)
        .parent()
        .ok_or_else(|| "HTML universe index has no parent".to_owned())?;
    let ledger_href = relative_url(index_parent, Path::new(LEDGER_HTML));
    let mut cards = String::new();
    for source in sources {
        let markdown = fs::read_to_string(repository_path(source))
            .map_err(|error| format!("could not read {}: {error}", source.display()))?;
        let title = document_title(&markdown, source);
        let category = category(source);
        let href = relative_url(index_parent, &mirror_path(source));
        write!(
            cards,
            "<a class=\"card\" data-search=\"{} {} {}\" href=\"{}\">\
             <span>{}</span><strong>{}</strong><code>{}</code></a>",
            html_escape(&title.to_lowercase()),
            html_escape(&path_text(source).to_lowercase()),
            html_escape(&category.to_lowercase()),
            html_escape(&href),
            html_escape(category),
            html_escape(&title),
            html_escape(&path_text(source))
        )
        .expect("write to string");
    }
    Ok(format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>rustmatch HTML evidence universe</title><style>{INDEX_CSS}</style></head>\
         <body><header><div class=\"kicker\">rustmatch · retained knowledge</div>\
         <h1>HTML evidence universe</h1><p>Human-readable mirrors of the repository's \
         Markdown evidence, protocols, decisions, and project documentation.</p>\
         <div class=\"actions\"><a href=\"{ledger_href}\">Optimization ledger</a>\
         <label>Filter documents<input id=\"filter\" type=\"search\" \
         placeholder=\"Try cache, parallel, ADR…\"></label></div></header>\
         <main><div class=\"notice\"><b>Audit-scale exception.</b> \
         <code>docs/optimization-and-scale.md</code> is a 33 MB machine-verifiable \
         evidence appendix and is intentionally not duplicated. Its human conclusions \
         and experiment history are presented in the optimization ledger.</div>\
         <div id=\"cards\" class=\"cards\">{cards}</div></main>\
         <footer>Generated by <code>cargo xtask optimization-ledger</code>; CI rejects \
         stale mirrors.</footer><script>{FILTER_SCRIPT}</script></body></html>"
    ))
}

fn category(path: &Path) -> &'static str {
    if path.starts_with("docs/evidence") {
        "Evidence package"
    } else if path.starts_with("docs/experiments") {
        "Experiment protocol"
    } else if path.starts_with("docs/adr") {
        "Architecture decision"
    } else if path.starts_with("docs/benchmarking") {
        "Benchmarking"
    } else if path.starts_with("docs") {
        "Project map"
    } else if path.starts_with(".github") {
        "Repository workflow"
    } else {
        "Project documentation"
    }
}

fn collect_files(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(repository_path(directory))
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("could not inspect {}: {error}", directory.display()))?;
        let path = normalize_path(&directory.join(entry.file_name()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_files(&path, output)?;
        } else if file_type.is_file() {
            output.push(path);
        }
    }
    Ok(())
}

fn repository_path(path: &Path) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest must be inside the repository")
        .join(path)
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const HEADING_SCRIPT: &str = r##"
const seen=new Map();
for(const heading of document.querySelectorAll("main h1,main h2,main h3,main h4")){
  const base=heading.textContent.toLowerCase().trim().replace(/[^a-z0-9]+/g,"-").replace(/^-|-$/g,"")||"section";
  const count=seen.get(base)||0; seen.set(base,count+1);
  heading.id=count?`${base}-${count}`:base;
  const anchor=document.createElement("a"); anchor.className="anchor"; anchor.href=`#${heading.id}`; anchor.textContent="#";
  anchor.setAttribute("aria-label",`Link to ${heading.textContent}`); heading.append(anchor);
}
"##;

const FILTER_SCRIPT: &str = r##"
const input=document.querySelector("#filter");
const cards=[...document.querySelectorAll(".card")];
input.addEventListener("input",()=>{const query=input.value.toLowerCase().trim();for(const card of cards){card.hidden=query&&!card.dataset.search.includes(query);}});
"##;

const DOCUMENT_CSS: &str = r#"
:root{--ink:#142821;--muted:#60736b;--paper:#f7f1e4;--cream:#fffaf0;--green:#0b6b5d;--rust:#b84c31;--line:#ccd6ce}
*{box-sizing:border-box}html{scroll-behavior:smooth}body{margin:0;color:var(--ink);background:radial-gradient(circle at 8% 0,#f1d5b8 0,transparent 28rem),linear-gradient(145deg,var(--paper),#e3eee8);font-family:"Iowan Old Style","Palatino Linotype",Georgia,serif;line-height:1.65}
nav{position:sticky;top:0;z-index:5;display:flex;justify-content:space-between;align-items:center;gap:24px;padding:15px max(24px,calc((100vw - 1040px)/2));background:rgba(247,241,228,.92);backdrop-filter:blur(16px);border-bottom:1px solid rgba(20,40,33,.16);font:700 .78rem "Avenir Next","Gill Sans",sans-serif;text-transform:uppercase;letter-spacing:.08em}nav div{display:flex;gap:18px;flex-wrap:wrap}a{color:var(--green);font-weight:700}.brand{text-decoration:none;color:var(--rust)}
header,main,footer{width:min(960px,calc(100% - 40px));margin:auto}header{padding:60px 0 28px}.kicker{font:800 .75rem "Avenir Next",sans-serif;text-transform:uppercase;letter-spacing:.13em;color:var(--rust)}.path{margin-top:8px;color:var(--muted);font:600 .82rem "SFMono-Regular",Consolas,monospace}
main{background:rgba(255,250,240,.82);border:1px solid var(--line);border-radius:28px;padding:clamp(28px,6vw,72px);box-shadow:0 24px 60px rgba(20,40,33,.09)}h1,h2,h3,h4{line-height:1.12;letter-spacing:-.025em;scroll-margin-top:90px}h1{font-size:clamp(2.7rem,7vw,5.4rem);margin-top:0}h2{font-size:clamp(1.9rem,4vw,3rem);border-top:1px solid var(--line);padding-top:1.2em;margin-top:1.8em}h3{font-size:1.55rem;margin-top:1.7em}h4{font-size:1.2rem}.anchor{font:700 .7em "Avenir Next",sans-serif;text-decoration:none;opacity:0;margin-left:.45em}.anchor:hover,h1:hover .anchor,h2:hover .anchor,h3:hover .anchor,h4:hover .anchor{opacity:1}
p,li{font-size:1.05rem}blockquote{margin:1.5em 0;padding:1px 22px;border-left:5px solid var(--rust);background:#f5e8d7;color:#445a51}code{font:.86em "SFMono-Regular",Consolas,monospace;background:#e9eee8;border-radius:5px;padding:.12em .32em}pre{overflow:auto;padding:20px;border-radius:15px;background:#142821;color:#eef4ed;line-height:1.45}pre code{background:none;padding:0;color:inherit}table{display:block;overflow-x:auto;border-collapse:collapse;width:100%;margin:1.5em 0}th,td{padding:11px 14px;border:1px solid var(--line);text-align:left}th{background:#e5ede7;font:700 .8rem "Avenir Next",sans-serif}img,svg{display:block;max-width:100%;height:auto;margin:24px auto;border-radius:12px}.mermaid-diagram{margin:30px 0;padding:24px;border:1px solid var(--line);border-radius:18px;background:rgba(255,255,255,.65)}.mermaid-diagram>a{display:block}.mermaid-diagram img{width:100%;margin:0 auto}.mermaid-diagram figcaption{margin-top:16px;color:var(--muted);font:700 .76rem "Avenir Next",sans-serif;text-align:center;text-transform:uppercase;letter-spacing:.08em}hr{border:0;border-top:1px solid var(--line);margin:2.5em 0}footer{padding:34px 0 70px;color:var(--muted)}
@media(max-width:720px){nav{position:static;align-items:flex-start;flex-direction:column}nav div{gap:10px}header{padding-top:34px}main{padding:26px 20px;border-radius:18px}h1{font-size:2.5rem}}
"#;

const INDEX_CSS: &str = r#"
:root{--ink:#10251f;--muted:#5d7169;--paper:#f5eddd;--cream:#fffaf0;--green:#0b6b5d;--rust:#b84c31;--line:#c9d5cd}
*{box-sizing:border-box}body{margin:0;color:var(--ink);background:radial-gradient(circle at 10% 3%,#f6cfaa 0,transparent 31rem),linear-gradient(145deg,var(--paper),#dcebe4);font-family:"Iowan Old Style","Palatino Linotype",Georgia,serif;line-height:1.55}header,main,footer{width:min(1160px,calc(100% - 40px));margin:auto}header{padding:78px 0 48px;border-bottom:1px solid rgba(16,37,31,.2)}.kicker,.card span,label{font:800 .75rem "Avenir Next","Gill Sans",sans-serif;text-transform:uppercase;letter-spacing:.12em;color:var(--rust)}h1{font-size:clamp(3.1rem,8vw,7.2rem);line-height:.9;letter-spacing:-.055em;margin:.18em 0}header p{font-size:1.28rem;max-width:730px;color:var(--muted)}.actions{display:flex;gap:24px;align-items:end;justify-content:space-between;margin-top:34px}.actions>a{display:inline-block;background:var(--ink);color:#fff8e9;padding:13px 18px;border-radius:999px;text-decoration:none;font:700 .78rem "Avenir Next",sans-serif;text-transform:uppercase;letter-spacing:.09em}label{display:grid;gap:7px;color:var(--ink)}input{width:min(420px,70vw);padding:13px 16px;border:1px solid var(--line);border-radius:12px;background:rgba(255,250,240,.88);font:1rem "Avenir Next",sans-serif;color:var(--ink)}main{padding:52px 0}.notice{padding:22px 25px;border:1px solid #ddba99;border-radius:16px;background:#f6e3cf;margin-bottom:30px}.cards{display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:17px}.card{display:flex;flex-direction:column;min-height:180px;padding:23px;border:1px solid var(--line);border-radius:20px;background:rgba(255,250,240,.82);color:var(--ink);text-decoration:none;box-shadow:0 12px 28px rgba(16,37,31,.06);transition:transform .18s ease,box-shadow .18s ease}.card:hover{transform:translateY(-4px);box-shadow:0 18px 34px rgba(16,37,31,.12)}.card strong{font-size:1.3rem;line-height:1.15;margin:15px 0 auto}.card code{font:.72rem "SFMono-Regular",Consolas,monospace;color:var(--muted);overflow-wrap:anywhere}.card[hidden]{display:none}footer{padding:30px 0 70px;border-top:1px solid rgba(16,37,31,.18);color:var(--muted)}code{font-family:"SFMono-Regular",Consolas,monospace}
@media(max-width:850px){.cards{grid-template-columns:repeat(2,minmax(0,1fr))}}@media(max-width:600px){header{padding-top:48px}.actions{align-items:stretch;flex-direction:column}.cards{grid-template-columns:1fr}.card{min-height:150px}input{width:100%}}
"#;

#[cfg(test)]
mod tests {
    use super::{
        bind_svg_to_source, ledger_evidence_href, markdown_sources, mermaid_blocks,
        mermaid_diagrams, mermaid_hash, mirror_path, normalize_path, relative_url,
        render_mermaid_blocks, split_local_url, verify_svg_source_binding,
    };
    use std::path::Path;

    #[test]
    fn ledger_routes_local_markdown_into_html_universe() {
        assert_eq!(
            ledger_evidence_href("evidence/i6/406c1f5/README.md"),
            "html-universe/repository/docs/evidence/i6/406c1f5/README.html"
        );
    }

    #[test]
    fn ledger_preserves_external_markdown_links() {
        let url = "https://github.com/example/project/blob/main/note.md";
        assert_eq!(ledger_evidence_href(url), url);
    }

    #[test]
    fn relative_urls_preserve_the_repository_shape() {
        assert_eq!(
            relative_url(
                Path::new("docs/html-universe/repository/docs/evidence/i7/37f6981"),
                &mirror_path(Path::new("docs/experiments/i7-safe-prefilter.md"))
            ),
            "../../../experiments/i7-safe-prefilter.html"
        );
    }

    #[test]
    fn paths_cannot_escape_the_repository() {
        assert!(normalize_path(Path::new("../../outside")).is_err());
    }

    #[test]
    fn local_url_suffixes_are_retained() {
        assert_eq!(
            split_local_url("../roadmap.md#milestones"),
            Some(("../roadmap.md", "#milestones"))
        );
    }

    #[test]
    fn mermaid_blocks_become_hash_bound_svg_figures() {
        let source = "flowchart LR\n    A --> B";
        let markdown = format!("# Document\n\n## Dependency graph\n\n```mermaid\n{source}\n```\n");
        let rendered =
            render_mermaid_blocks(&markdown, Path::new("docs/html-universe/repository/docs"))
                .expect("render Mermaid block");
        let hash = mermaid_hash(source);

        assert!(!rendered.contains("```mermaid"));
        assert!(rendered.contains("<figure class=\"mermaid-diagram\">"));
        assert!(rendered.contains("alt=\"Dependency graph diagram\""));
        assert!(rendered.contains(&format!("../../assets/mermaid/{hash}.svg")));
    }

    #[test]
    fn rendered_svg_is_bound_to_its_source_hash() {
        let hash = mermaid_hash("flowchart LR\nA --> B");
        let svg = bind_svg_to_source("<svg></svg>".to_owned(), &hash).expect("bind SVG");
        assert!(verify_svg_source_binding(svg.as_bytes(), &hash, Path::new("diagram.svg")).is_ok());
        assert!(
            verify_svg_source_binding(
                svg.as_bytes(),
                &mermaid_hash("different"),
                Path::new("diagram.svg")
            )
            .is_err()
        );
    }

    #[test]
    fn repository_mermaid_inventory_is_complete() {
        let sources = markdown_sources().expect("collect Markdown");
        let diagrams = mermaid_diagrams(&sources).expect("collect Mermaid diagrams");
        assert_eq!(diagrams.len(), 4);

        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("repository root");
        let roadmap =
            std::fs::read_to_string(repository_root.join("docs/roadmap.md")).expect("read roadmap");
        assert_eq!(mermaid_blocks(&roadmap).expect("parse roadmap").len(), 1);
        let input_parallel = std::fs::read_to_string(
            repository_root.join("docs/experiments/b2-h-0006-input-parallel-design.md"),
        )
        .expect("read input-parallel design");
        assert_eq!(
            mermaid_blocks(&input_parallel)
                .expect("parse input-parallel design")
                .len(),
            1
        );
        let portfolio = std::fs::read_to_string(
            repository_root.join("docs/experiments/post-h11-future-optimization-portfolio.md"),
        )
        .expect("read post-H11 portfolio");
        assert_eq!(
            mermaid_blocks(&portfolio)
                .expect("parse post-H11 portfolio")
                .len(),
            1
        );
    }
}
