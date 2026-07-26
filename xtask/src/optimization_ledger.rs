//! Generate the optimization-attempt ledger and its progress chart.

use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const SOURCE: &str = "docs/optimization-attempts.json";
const MARKDOWN: &str = "docs/optimization-attempt-ledger.md";
const HTML: &str = "docs/optimization-attempt-ledger.html";
const SVG: &str = "docs/plots/optimization-progress.svg";

#[derive(Debug, Deserialize)]
struct Ledger {
    schema_version: u32,
    title: String,
    last_reviewed: String,
    policy: String,
    attempts: Vec<Attempt>,
    comparison_snapshots: Vec<ComparisonSnapshot>,
    future_candidates: Vec<FutureCandidate>,
}

#[derive(Debug, Deserialize)]
struct Attempt {
    id: String,
    name: String,
    date: String,
    status: String,
    baseline_revision: String,
    candidate_revision: String,
    summary: String,
    result: String,
    tradeoffs: String,
    decision: String,
    evidence: String,
    progress: Option<Progress>,
}

#[derive(Debug, Deserialize)]
struct Progress {
    workload: String,
    baseline_millis: f64,
    candidate_millis: f64,
}

#[derive(Debug, Deserialize)]
struct ComparisonSnapshot {
    date: String,
    rust_revision: String,
    review_revision: String,
    method: String,
    engines: Vec<EngineComparison>,
}

#[derive(Debug, Deserialize)]
struct EngineComparison {
    engine: String,
    semantics: String,
    cells: u32,
    rust_wins: u32,
    geometric_mean_ratio: f64,
    median_ratio: f64,
    minimum_ratio: f64,
    maximum_ratio: f64,
    evidence: String,
}

#[derive(Debug, Deserialize)]
struct FutureCandidate {
    priority: u32,
    id: String,
    name: String,
    status: String,
    why: String,
    next_test: String,
    expected_value: String,
}

#[derive(Debug)]
struct ProgressPoint<'a> {
    label: &'a str,
    workload: &'a str,
    incremental_speedup: f64,
    cumulative_speedup: f64,
}

pub fn render() -> Result<(), String> {
    let generated = generate()?;
    write_output(MARKDOWN, &generated.markdown)?;
    write_output(HTML, &generated.html)?;
    write_output(SVG, &generated.svg)?;
    crate::html_universe::render()?;
    println!("rendered {MARKDOWN}, {HTML}, and {SVG}");
    Ok(())
}

pub fn verify() -> Result<(), String> {
    let generated = generate()?;
    verify_output(MARKDOWN, &generated.markdown)?;
    verify_output(HTML, &generated.html)?;
    verify_output(SVG, &generated.svg)?;
    crate::html_universe::verify()?;
    println!("optimization ledger is current");
    Ok(())
}

struct Generated {
    markdown: String,
    html: String,
    svg: String,
}

fn generate() -> Result<Generated, String> {
    let source =
        fs::read_to_string(SOURCE).map_err(|error| format!("could not read {SOURCE}: {error}"))?;
    let ledger: Ledger = serde_json::from_str(&source)
        .map_err(|error| format!("could not parse {SOURCE}: {error}"))?;
    validate(&ledger)?;
    let progress = progress_points(&ledger);
    let svg = render_svg(&progress)?;
    Ok(Generated {
        markdown: render_markdown(&ledger, &progress),
        html: render_html(&ledger, &progress, &svg),
        svg,
    })
}

fn validate(ledger: &Ledger) -> Result<(), String> {
    if ledger.schema_version != 1 {
        return Err(format!(
            "unsupported ledger schema {}",
            ledger.schema_version
        ));
    }
    if ledger.attempts.is_empty() {
        return Err("optimization ledger has no attempts".to_owned());
    }
    let mut identifiers = HashSet::new();
    for attempt in &ledger.attempts {
        if !identifiers.insert(&attempt.id) {
            return Err(format!("duplicate optimization attempt ID {}", attempt.id));
        }
        if !matches!(
            attempt.status.as_str(),
            "merged" | "rejected" | "inconclusive"
        ) {
            return Err(format!(
                "optimization attempt {} has invalid status {}",
                attempt.id, attempt.status
            ));
        }
        validate_revision(&attempt.baseline_revision, &attempt.id)?;
        validate_revision(&attempt.candidate_revision, &attempt.id)?;
        if attempt.status == "merged" {
            let progress = attempt
                .progress
                .as_ref()
                .ok_or_else(|| format!("merged attempt {} has no progress metric", attempt.id))?;
            if !progress.baseline_millis.is_finite()
                || !progress.candidate_millis.is_finite()
                || progress.baseline_millis <= 0.0
                || progress.candidate_millis <= 0.0
                || progress.candidate_millis >= progress.baseline_millis
            {
                return Err(format!(
                    "merged attempt {} does not contain a positive time improvement",
                    attempt.id
                ));
            }
        } else if attempt.progress.is_some() {
            return Err(format!(
                "non-merged attempt {} may not enter the progress series",
                attempt.id
            ));
        }
    }
    let mut priorities = ledger
        .future_candidates
        .iter()
        .map(|candidate| candidate.priority)
        .collect::<Vec<_>>();
    priorities.sort_unstable();
    let expected = (1..=u32::try_from(priorities.len())
        .map_err(|_| "too many future candidates")?)
        .collect::<Vec<_>>();
    if priorities != expected {
        return Err("future candidate priorities must be contiguous from one".to_owned());
    }
    for snapshot in &ledger.comparison_snapshots {
        validate_revision(&snapshot.rust_revision, "comparison snapshot")?;
        validate_revision(&snapshot.review_revision, "comparison snapshot")?;
        for engine in &snapshot.engines {
            if engine.cells == 0
                || engine.rust_wins > engine.cells
                || !engine.geometric_mean_ratio.is_finite()
                || engine.geometric_mean_ratio <= 0.0
                || engine.minimum_ratio <= 0.0
                || engine.minimum_ratio > engine.maximum_ratio
            {
                return Err(format!(
                    "comparison snapshot has invalid {} metrics",
                    engine.engine
                ));
            }
        }
    }
    Ok(())
}

fn validate_revision(revision: &str, label: &str) -> Result<(), String> {
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!("{label} has an invalid Git revision: {revision}"));
    }
    Ok(())
}

fn progress_points(ledger: &Ledger) -> Vec<ProgressPoint<'_>> {
    let mut cumulative = 1.0;
    ledger
        .attempts
        .iter()
        .filter_map(|attempt| {
            let progress = attempt.progress.as_ref()?;
            let incremental = progress.baseline_millis / progress.candidate_millis;
            cumulative *= incremental;
            Some(ProgressPoint {
                label: &attempt.id,
                workload: &progress.workload,
                incremental_speedup: incremental,
                cumulative_speedup: cumulative,
            })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn render_markdown(ledger: &Ledger, progress: &[ProgressPoint<'_>]) -> String {
    let mut output = String::new();
    writeln!(output, "# {}\n", ledger.title).expect("write to string");
    writeln!(
        output,
        "> Generated from [`optimization-attempts.json`](optimization-attempts.json). \
         Do not edit this file directly. Last reviewed: **{}**.\n",
        ledger.last_reviewed
    )
    .expect("write to string");
    writeln!(output, "## Admission policy\n\n{}\n", ledger.policy).expect("write to string");
    writeln!(
        output,
        "## Merged-improvement progress\n\n![Optimization progress](plots/optimization-progress.svg)\n"
    )
    .expect("write to string");
    writeln!(
        output,
        "The chart is an **admission evidence speedup index**, not a direct \
         end-to-end historical benchmark. It starts at 1.0 and multiplies each \
         merged optimization's frozen baseline/candidate scan-time ratio. \
         Rejected and inconclusive experiments are excluded by construction.\n"
    )
    .expect("write to string");
    writeln!(
        output,
        "| Merged optimization | Frozen positive gate | Incremental speedup | Cumulative index |\n\
         |---|---|---:|---:|"
    )
    .expect("write to string");
    for point in progress {
        writeln!(
            output,
            "| {} | {} | {:.2}x | {:.2}x |",
            point.label, point.workload, point.incremental_speedup, point.cumulative_speedup
        )
        .expect("write to string");
    }

    writeln!(output, "\n## Attempt ledger\n").expect("write to string");
    for attempt in &ledger.attempts {
        writeln!(
            output,
            "### {} - {}\n\n\
             - **Date:** {}\n\
             - **Outcome:** `{}`\n\
             - **Baseline:** `{}`\n\
             - **Candidate:** `{}`\n\
             - **Mechanism:** {}\n\
             - **Measured result:** {}\n\
             - **Tradeoffs and caveats:** {}\n\
             - **Decision:** {}\n\
             - **Evidence:** [{}]({})\n",
            attempt.id,
            attempt.name,
            attempt.date,
            attempt.status,
            attempt.baseline_revision,
            attempt.candidate_revision,
            attempt.summary,
            attempt.result,
            attempt.tradeoffs,
            attempt.decision,
            attempt.evidence,
            attempt.evidence
        )
        .expect("write to string");
    }

    writeln!(output, "## Cross-engine evolution\n").expect("write to string");
    for snapshot in &ledger.comparison_snapshots {
        writeln!(
            output,
            "### Snapshot {}\n\nRust revision `{}`; reviewed measurement revision \
             `{}`.\n\n{}\n",
            snapshot.date, snapshot.rust_revision, snapshot.review_revision, snapshot.method
        )
        .expect("write to string");
        writeln!(
            output,
            "| Engine | Contract | Cells won | Geometric-mean Rust/engine | Median | Range |\n\
             |---|---|---:|---:|---:|---:|"
        )
        .expect("write to string");
        for engine in &snapshot.engines {
            writeln!(
                output,
                "| [{}]({}) | {} | {}/{} | {:.3}x | {:.3}x | {:.3}x-{:.3}x |",
                engine.engine,
                engine.evidence,
                engine.semantics,
                engine.rust_wins,
                engine.cells,
                engine.geometric_mean_ratio,
                engine.median_ratio,
                engine.minimum_ratio,
                engine.maximum_ratio
            )
            .expect("write to string");
        }
    }

    writeln!(output, "\n## Plausible future optimizations\n").expect("write to string");
    writeln!(
        output,
        "| Priority | Hypothesis | Status | Why it remains plausible | Next discriminating test | Expected value |\n\
         |---:|---|---|---|---|---|"
    )
    .expect("write to string");
    for candidate in &ledger.future_candidates {
        writeln!(
            output,
            "| {} | **{} - {}** | {} | {} | {} | {} |",
            candidate.priority,
            candidate.id,
            candidate.name,
            candidate.status,
            candidate.why,
            candidate.next_test,
            candidate.expected_value
        )
        .expect("write to string");
    }
    writeln!(
        output,
        "\n## Regeneration\n\nRun `cargo xtask optimization-ledger` after changing \
         the source ledger. `cargo xtask ci` runs \
         `cargo xtask verify-optimization-ledger` and fails when the Markdown, \
         HTML, SVG, or HTML evidence universe is stale."
    )
    .expect("write to string");
    output
}

#[allow(clippy::too_many_lines)]
fn render_html(ledger: &Ledger, progress: &[ProgressPoint<'_>], svg: &str) -> String {
    let mut attempts = String::new();
    for attempt in &ledger.attempts {
        let status_class = match attempt.status.as_str() {
            "merged" => "merged",
            "rejected" => "rejected",
            _ => "inconclusive",
        };
        let evidence_href = crate::html_universe::ledger_evidence_href(&attempt.evidence);
        write!(
            attempts,
            "<article class=\"attempt\"><div class=\"attempt-head\"><div><span class=\"eyebrow\">{}</span>\
             <h3>{}</h3></div><span class=\"status {}\">{}</span></div>\
             <p>{}</p><dl><dt>Measured result</dt><dd>{}</dd>\
             <dt>Tradeoffs</dt><dd>{}</dd><dt>Decision</dt><dd>{}</dd></dl>\
             <p class=\"meta\">{} · baseline <code>{}</code> · candidate <code>{}</code></p>\
             <a href=\"{}\">Open retained evidence</a></article>",
            html_escape(&attempt.id),
            html_escape(&attempt.name),
            status_class,
            html_escape(&attempt.status),
            html_escape(&attempt.summary),
            html_escape(&attempt.result),
            html_escape(&attempt.tradeoffs),
            html_escape(&attempt.decision),
            html_escape(&attempt.date),
            html_escape(short_revision(&attempt.baseline_revision)),
            html_escape(short_revision(&attempt.candidate_revision)),
            html_escape(&evidence_href)
        )
        .expect("write to string");
    }

    let mut progress_rows = String::new();
    for point in progress {
        write!(
            progress_rows,
            "<tr><th>{}</th><td>{}</td><td>{:.2}×</td><td>{:.2}×</td></tr>",
            html_escape(point.label),
            html_escape(point.workload),
            point.incremental_speedup,
            point.cumulative_speedup
        )
        .expect("write to string");
    }

    let mut comparisons = String::new();
    for snapshot in &ledger.comparison_snapshots {
        let mut cards = String::new();
        for engine in &snapshot.engines {
            let relation = if engine.geometric_mean_ratio >= 1.0 {
                format!("{:.2}× Rust lead", engine.geometric_mean_ratio)
            } else {
                format!("{:.2}× competitor lead", 1.0 / engine.geometric_mean_ratio)
            };
            write!(
                cards,
                "<a class=\"engine-card\" href=\"{}\"><span>{}</span><strong>{:.3}×</strong>\
                 <small>Rust/engine geometric mean</small><b>{}</b>\
                 <em>{}/{} cells won · {}</em></a>",
                html_escape(&engine.evidence),
                html_escape(&engine.engine),
                engine.geometric_mean_ratio,
                html_escape(&relation),
                engine.rust_wins,
                engine.cells,
                html_escape(&engine.semantics)
            )
            .expect("write to string");
        }
        write!(
            comparisons,
            "<section class=\"snapshot\"><div class=\"section-kicker\">Snapshot {}</div>\
             <h2>Rust against the field</h2><p>{}</p><div class=\"engine-grid\">{}</div>\
             <p class=\"meta\">Rust <code>{}</code> · review <code>{}</code></p></section>",
            html_escape(&snapshot.date),
            html_escape(&snapshot.method),
            cards,
            html_escape(short_revision(&snapshot.rust_revision)),
            html_escape(short_revision(&snapshot.review_revision))
        )
        .expect("write to string");
    }

    let mut candidates = String::new();
    for candidate in &ledger.future_candidates {
        write!(
            candidates,
            "<li><span class=\"rank\">{:02}</span><div><div class=\"candidate-title\">{} · {}</div>\
             <p>{}</p><p><b>Next test:</b> {}</p><small>{} · {}</small></div></li>",
            candidate.priority,
            html_escape(&candidate.id),
            html_escape(&candidate.name),
            html_escape(&candidate.why),
            html_escape(&candidate.next_test),
            html_escape(&candidate.status),
            html_escape(&candidate.expected_value)
        )
        .expect("write to string");
    }

    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title><style>{css}</style></head><body>\
         <header><div class=\"eyebrow\">rustmatch · performance evidence</div>\
         <h1>{title}</h1><p class=\"lede\">A proof-oriented history of what made \
         Rustmatch faster, what failed, and where it stands against Java rmatch, \
         RegexSet, and Hyperscan.</p><div class=\"top-links\">\
         <a href=\"html-universe/index.html\">Browse the HTML evidence universe</a></div>\
         <div class=\"stamp\">Reviewed {reviewed}</div></header>\
         <main><section class=\"policy\"><div class=\"section-kicker\">Non-negotiable gate</div>\
         <p>{policy}</p></section><section><div class=\"section-kicker\">Merged only</div>\
         <h2>Efficiency progress</h2><div class=\"chart\">{svg}</div>\
         <p class=\"chart-note\">Admission evidence speedup index on a logarithmic scale. \
         Each point multiplies the prior index by that merged optimization's own \
         frozen baseline/candidate scan-time ratio. It is not a direct historical rerun.</p>\
         <div class=\"table-wrap\"><table><thead><tr><th>Optimization</th><th>Frozen gate</th>\
         <th>Incremental</th><th>Cumulative</th></tr></thead><tbody>{progress_rows}</tbody>\
         </table></div></section><section><div class=\"section-kicker\">Complete record</div>\
         <h2>Optimization attempts</h2><div class=\"attempts\">{attempts}</div></section>\
         {comparisons}<section><div class=\"section-kicker\">Evidence-ranked queue</div>\
         <h2>Plausible future optimizations</h2><ol class=\"candidate-list\">{candidates}</ol>\
         </section><footer>Generated from <code>docs/optimization-attempts.json</code>. \
         <a href=\"html-universe/index.html\">Browse retained documents as HTML</a>. \
         Run <code>cargo xtask optimization-ledger</code>; CI rejects stale output.</footer>\
         </main></body></html>",
        title = html_escape(&ledger.title),
        reviewed = html_escape(&ledger.last_reviewed),
        policy = html_escape(&ledger.policy),
        css = CSS,
    )
}

fn render_svg(progress: &[ProgressPoint<'_>]) -> Result<String, String> {
    if progress.is_empty() {
        return Err("optimization progress chart has no merged improvements".to_owned());
    }
    let width = 1_120.0;
    let height = 560.0;
    let left = 92.0;
    let right = 46.0;
    let top = 64.0;
    let bottom = 112.0;
    let plot_width = width - left - right;
    let plot_height = height - top - bottom;
    let maximum = progress
        .iter()
        .map(|point| point.cumulative_speedup)
        .fold(1.0_f64, f64::max);
    let maximum_power = maximum.log10().ceil().max(1.0);
    let point_count = u32::try_from(progress.len() + 1).map_err(|_| "too many progress points")?;
    let denominator = f64::from(point_count - 1);

    let mut grid = String::new();
    let mut power = 0_u32;
    while f64::from(power) <= maximum_power {
        let y = top + plot_height * (1.0 - f64::from(power) / maximum_power);
        let value = 10_u64.pow(power);
        write!(
            grid,
            "<line x1=\"{left}\" y1=\"{y:.2}\" x2=\"{}\" y2=\"{y:.2}\" class=\"grid\"/>\
             <text x=\"{}\" y=\"{:.2}\" class=\"tick\" text-anchor=\"end\">{}×</text>",
            width - right,
            left - 16.0,
            y + 5.0,
            value
        )
        .expect("write to string");
        power = power
            .checked_add(1)
            .ok_or_else(|| "optimization progress scale is too large".to_owned())?;
    }

    let mut coordinates = vec![(left, top + plot_height)];
    for (index, point) in progress.iter().enumerate() {
        let ordinal = u32::try_from(index + 1).map_err(|_| "too many progress points")?;
        let x = left + plot_width * f64::from(ordinal) / denominator;
        let y = top + plot_height * (1.0 - point.cumulative_speedup.log10() / maximum_power);
        coordinates.push((x, y));
    }
    let polyline = coordinates
        .iter()
        .map(|(x, y)| format!("{x:.2},{y:.2}"))
        .collect::<Vec<_>>()
        .join(" ");

    let mut points = String::new();
    for (index, (x, y)) in coordinates.iter().enumerate() {
        let (label, value) = if index == 0 {
            ("Pre-I6", "1.00×".to_owned())
        } else {
            let point = &progress[index - 1];
            (point.label, format!("{:.2}×", point.cumulative_speedup))
        };
        write!(
            points,
            "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"8\"/>\
             <text x=\"{x:.2}\" y=\"{:.2}\" class=\"value\" text-anchor=\"middle\">{}</text>\
             <text x=\"{x:.2}\" y=\"{}\" class=\"label\" text-anchor=\"middle\">{}</text>",
            y - 18.0,
            xml_escape(&value),
            height - 58.0,
            xml_escape(label)
        )
        .expect("write to string");
    }

    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1120 560\" \
         role=\"img\" aria-labelledby=\"title description\">\
         <title id=\"title\">Merged Rustmatch optimization progress</title>\
         <desc id=\"description\">Logarithmic cumulative admission evidence speedup \
         index containing only merged improvements.</desc><defs>\
         <linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
         <stop offset=\"0\" stop-color=\"#f4ead5\"/><stop offset=\"1\" stop-color=\"#d8e9df\"/>\
         </linearGradient><linearGradient id=\"line\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\">\
         <stop offset=\"0\" stop-color=\"#ba4b2f\"/><stop offset=\"1\" stop-color=\"#0b6b5d\"/>\
         </linearGradient><style>.grid{{stroke:#20332e;stroke-opacity:.14;stroke-dasharray:5 8}}\
         .tick,.label{{fill:#314b43;font:600 15px 'Avenir Next',sans-serif}}\
         .value{{fill:#10251f;font:700 17px 'Avenir Next',sans-serif}}\
         .axis-title{{fill:#314b43;font:600 14px 'Avenir Next',sans-serif;letter-spacing:.08em}}\
         circle{{fill:#fffaf0;stroke:#0b6b5d;stroke-width:5}}</style></defs>\
         <rect width=\"1120\" height=\"560\" rx=\"28\" fill=\"url(#bg)\"/>\
         <text x=\"92\" y=\"36\" class=\"axis-title\">CUMULATIVE SPEEDUP INDEX · LOG SCALE</text>\
         {grid}<polyline points=\"{polyline}\" fill=\"none\" stroke=\"url(#line)\" \
         stroke-width=\"7\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>\
         {points}</svg>"
    ))
}

fn short_revision(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn xml_escape(value: &str) -> String {
    html_escape(value).replace('\'', "&apos;")
}

fn write_output(path: &str, contents: &str) -> Result<(), String> {
    let output = Path::new(path);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    fs::write(output, contents).map_err(|error| format!("could not write {path}: {error}"))
}

fn verify_output(path: &str, expected: &str) -> Result<(), String> {
    let actual = fs::read_to_string(path)
        .map_err(|error| format!("{path} is missing; regenerate it: {error}"))?;
    if actual != expected {
        return Err(format!(
            "{path} is stale; run `cargo xtask optimization-ledger`"
        ));
    }
    Ok(())
}

const CSS: &str = r#"
:root{--ink:#10251f;--muted:#587067;--paper:#f6f0e2;--cream:#fffaf0;--green:#0b6b5d;--rust:#ba4b2f;--line:#c8d5cd}
*{box-sizing:border-box}body{margin:0;color:var(--ink);background:radial-gradient(circle at 8% 4%,#f7d8b6 0,transparent 27rem),linear-gradient(145deg,#f7f0df,#dcece4);font-family:"Iowan Old Style","Palatino Linotype",Georgia,serif;line-height:1.55}
header,main{width:min(1160px,calc(100% - 40px));margin:auto}header{padding:80px 0 52px;border-bottom:1px solid rgba(16,37,31,.2);position:relative}h1{font-size:clamp(3rem,8vw,7.2rem);line-height:.9;max-width:950px;margin:.15em 0;letter-spacing:-.055em}h2{font-size:clamp(2rem,4vw,3.5rem);line-height:1;margin:.2em 0 .7em}h3{font-size:1.5rem;margin:.15em 0}.eyebrow,.section-kicker,.status,.stamp,.meta,small,th,.rank{font-family:"Avenir Next","Gill Sans",sans-serif;text-transform:uppercase;letter-spacing:.1em}.eyebrow,.section-kicker{font-weight:700;color:var(--rust);font-size:.78rem}.lede{font-size:1.35rem;max-width:760px;color:var(--muted)}.top-links{margin-top:25px}.top-links a{display:inline-block;padding:10px 15px;border:1px solid var(--green);border-radius:999px;text-decoration:none;font:700 .72rem "Avenir Next",sans-serif;text-transform:uppercase;letter-spacing:.08em}.stamp{position:absolute;right:0;top:92px;font-size:.72rem}.policy{background:var(--ink);color:#f8f2e5;padding:32px 40px;border-radius:0 0 26px 26px}.policy p{font-size:1.2rem;margin:.4rem 0}.policy .section-kicker{color:#f5b38c}section{padding:66px 0}.chart{filter:drop-shadow(0 18px 24px rgba(16,37,31,.12))}.chart svg{display:block;width:100%;height:auto}.chart-note{max-width:860px;color:var(--muted)}.table-wrap{overflow-x:auto;background:rgba(255,250,240,.72);border:1px solid var(--line);border-radius:18px;margin-top:28px}table{border-collapse:collapse;width:100%}th,td{text-align:left;padding:16px 18px;border-bottom:1px solid var(--line)}th{font-size:.72rem}.attempts{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:20px}.attempt{background:rgba(255,250,240,.82);border:1px solid var(--line);border-radius:22px;padding:26px;box-shadow:0 14px 38px rgba(16,37,31,.07)}.attempt-head{display:flex;justify-content:space-between;gap:18px}.status{height:max-content;font-size:.65rem;padding:6px 9px;border-radius:99px}.status.merged{background:#cce9da;color:#075243}.status.rejected{background:#f2c9bc;color:#7a2818}.status.inconclusive{background:#efdfae;color:#654d06}dl{display:grid;grid-template-columns:130px 1fr;gap:10px 16px}dt{font-weight:700}dd{margin:0;color:var(--muted)}code{font:600 .86em "SFMono-Regular",Consolas,monospace}.meta{font-size:.68rem;color:var(--muted)}a{color:var(--green);font-weight:700}.snapshot{border-top:1px solid rgba(16,37,31,.2)}.engine-grid{display:grid;grid-template-columns:repeat(3,1fr);gap:18px}.engine-card{text-decoration:none;color:var(--ink);background:var(--cream);border:1px solid var(--line);border-radius:22px;padding:24px;display:flex;flex-direction:column;transition:transform .2s ease,box-shadow .2s ease}.engine-card:hover{transform:translateY(-4px);box-shadow:0 14px 28px rgba(16,37,31,.12)}.engine-card span{font:700 .8rem "Avenir Next",sans-serif;text-transform:uppercase;letter-spacing:.1em;color:var(--rust)}.engine-card strong{font:700 clamp(2.8rem,6vw,5rem) "Avenir Next",sans-serif;letter-spacing:-.06em}.engine-card b{margin-top:16px}.engine-card em{font-size:.88rem;color:var(--muted);margin-top:8px}.candidate-list{list-style:none;padding:0;border-top:1px solid var(--line)}.candidate-list li{display:grid;grid-template-columns:70px 1fr;gap:20px;padding:24px 0;border-bottom:1px solid var(--line)}.rank{font-size:2rem;color:var(--rust)}.candidate-title{font-weight:700;font-size:1.25rem}.candidate-list p{margin:.45rem 0;color:var(--muted)}footer{padding:36px 0 70px;border-top:1px solid rgba(16,37,31,.2);color:var(--muted)}
@media(max-width:760px){header{padding-top:50px}.stamp{position:static;margin-top:24px}.attempts,.engine-grid{grid-template-columns:1fr}.policy{padding:26px}.attempt-head{display:block}.status{display:inline-block;margin-top:10px}dl{grid-template-columns:1fr}.candidate-list li{grid-template-columns:46px 1fr}.rank{font-size:1.3rem}}
"#;

#[cfg(test)]
mod tests {
    use super::{Ledger, html_escape, progress_points, render_svg, validate};

    fn ledger() -> Ledger {
        serde_json::from_str(include_str!("../../docs/optimization-attempts.json"))
            .expect("source ledger should parse")
    }

    #[test]
    fn checked_in_ledger_is_valid() {
        assert_eq!(validate(&ledger()), Ok(()));
    }

    #[test]
    fn progress_excludes_rejected_attempts() {
        let ledger = ledger();
        let points = progress_points(&ledger);
        assert_eq!(
            points.iter().map(|point| point.label).collect::<Vec<_>>(),
            ["I6", "I7", "I8"]
        );
        let svg = render_svg(&points).expect("chart should render");
        assert!(!svg.contains("B2-H-0001"));
        assert!(!svg.contains("B2-H-0005"));
    }

    #[test]
    fn html_escapes_markup() {
        assert_eq!(
            html_escape("<unsafe & noisy>"),
            "&lt;unsafe &amp; noisy&gt;"
        );
    }
}
