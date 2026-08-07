use std::env;
use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::path::Path;

use rustmatch::{MatcherBuilder, PatternId};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let pattern_path = arguments.next().ok_or("missing pattern path")?;
    let mode = arguments
        .next()
        .ok_or("missing mode")?
        .into_string()
        .map_err(|_| "mode is not UTF-8")?;
    if arguments.next().is_some() {
        return Err("unexpected prepare-probe argument".into());
    }

    let patterns = fs::read_to_string(Path::new(&pattern_path))?;
    let mut builder = MatcherBuilder::new();
    let mut pattern_count = 0_usize;
    for (line_index, line) in patterns.lines().enumerate() {
        let (id, expression) = line
            .split_once('\t')
            .ok_or_else(|| format!("pattern line {} has no tab", line_index + 1))?;
        builder.add(PatternId::new(id.parse()?), expression)?;
        pattern_count += 1;
    }

    let diagnostics = match mode.as_str() {
        "generic" => builder
            .build_shared_cohort_diagnostic()?
            .structure_diagnostics(),
        "specialized" => builder
            .build_shared_cohort_assertion_diagnostic()?
            .structure_diagnostics(),
        _ => return Err(format!("unsupported prepare-probe mode {mode:?}").into()),
    };
    black_box(diagnostics);
    println!("{{\"schema_version\":1,\"mode\":\"{mode}\",\"pattern_count\":{pattern_count}}}");
    Ok(())
}
