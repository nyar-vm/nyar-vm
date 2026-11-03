//! Reproducible diagnostic baseline snapshot for the `std` test curve.
//!
//! Unique key = `(diagnostic id, first symbol in the header/notes)`. Every test entry
//! compiles the whole `std` library, so one key usually counts once per entry (131 for
//! the 2026-11-04 x `ovm` baseline).
//!
//! Usage (run from the `nyar-vm.rs` repository root):
//!
//! ```powershell
//! $env:CARGO_TARGET_DIR='e:\victory 胜利女神\nyar-vm.rs\.target-std-loop'
//! cargo +nightly-2026-10-23 run -q --manifest-path 'projects\nyar.rs\Cargo.toml' `
//!   -p rusty-valkyrie --example b_diagnostic_snapshot -- `
//!   'e:\victory 胜利女神\valkyrie.v\projects\std' ovm 'e:\victory 胜利女神\reports'
//! ```
//!
//! Outputs `reports/std-diagnostic-baseline-2026-11-04.{json,md}` next to the given
//! out dir: totals, per-key occurrence counts, first `file:line`, first entry, and an
//! attribution guess (`compiler` / `std` / `harness` / `unknown`). Pass a fourth
//! non-empty argument (for example `dump`) to print two raw error bodies for format checks.
//! Regenerate after every change and diff `groups` instead of counting messages by eye.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use rusty_valkyrie::legion::{cmds::test_engine::run_tests_for_project, workspace::planner::WorkspaceResolver};

#[derive(Default, Clone)]
struct Occurrence {
    id: String,
    symbol: String,
    count: usize,
    first_file: String,
    first_line: usize,
    first_entry: String,
    sample: String,
}

struct Diagnostic {
    id: String,
    symbol: String,
    file: String,
    line: usize,
    text: String,
}

fn first_backtick_symbol(text: &str) -> Option<String> {
    let start = text.find('`')?;
    let rest = &text[start + 1..];
    let end = rest.find('`')?;
    let symbol = rest[..end].trim();
    (!symbol.is_empty()).then(|| symbol.to_string())
}

/// `--> path:line:col` / `path:line:col` 位置行解析。路径允许含盘符冒号，所以从尾部取两个数字段。
fn parse_location(line: &str) -> Option<(String, usize)> {
    let trimmed = line.trim_start().trim_start_matches("-->").trim();
    let parts = trimmed.split(':').collect::<Vec<_>>();
    if parts.len() < 3 {
        return None;
    }
    let column = parts.last()?;
    let line_number = parts.get(parts.len() - 2)?;
    if !column.chars().take_while(|ch| ch.is_ascii_digit()).count().gt(&0) {
        return None;
    }
    let line: usize = line_number.parse().ok()?;
    let path = parts[..parts.len() - 2].join(":");
    Some((path, line))
}

fn parse_diagnostics(message: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut current: Option<(String, Vec<String>)> = None;
    let mut finish = |current: Option<(String, Vec<String>)>, diagnostics: &mut Vec<Diagnostic>| {
        let Some((header, lines)) = current
        else {
            return;
        };
        let (id, message) = {
            let Some(rest) = header.strip_prefix("error[") else {
                return;
            };
            let Some(end) = rest.find(']') else {
                return;
            };
            let id = rest[..end].to_string();
            let message = rest[end + 1..].trim_start_matches(':').trim().to_string();
            (id, message)
        };
        let symbol = first_backtick_symbol(&message)
            .or_else(|| lines.iter().find_map(|line| first_backtick_symbol(line)))
            .unwrap_or_default();
        let (file, line) = lines
            .iter()
            .find_map(|line| parse_location(line))
            .unwrap_or_else(|| (String::new(), 0));
        let text = std::iter::once(header.clone()).chain(lines.clone()).collect::<Vec<_>>().join("\n");
        diagnostics.push(Diagnostic { id, symbol, file, line, text });
    };
    for line in message.lines() {
        if line.trim_start().starts_with("error[") {
            finish(current.take(), &mut diagnostics);
            current = Some((line.trim_start().to_string(), Vec::new()));
        }
        else if let Some((_, lines)) = current.as_mut() {
            lines.push(line.to_string());
        }
    }
    finish(current.take(), &mut diagnostics);
    diagnostics
}

fn attribution_guess(diagnostic: &Diagnostic) -> &'static str {
    let id = diagnostic.id.as_str();
    let file = diagnostic.file.replace('\\', "/");
    if id.starts_with("valkyrie.legion") || id.contains("workspace") || id.contains("planner") {
        return "harness";
    }
    if file.starts_with("source/") || file.contains("/source/") {
        // std 源码面错误（缺声明、缺合同、来源形状）归 std；作用域/解析引擎类仍归编译器。
        return match id {
            "valkyrie.hir.unresolved_call"
            | "valkyrie.hir.unresolved_constructor"
            | "valkyrie.hir.no_method_candidate"
            | "valkyrie.hir.ambiguous_call" => "compiler",
            _ => "std",
        };
    }
    if id.starts_with("valkyrie.hir")
        || id.starts_with("valkyrie.mir")
        || id.starts_with("valkyrie.syntax")
        || id.starts_with("valkyrie.emitter")
        || id.starts_with("CAN")
        || id.starts_with("SMIR")
        || id.starts_with("CMIR")
    {
        return "compiler";
    }
    "unknown"
}

fn json_escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let project = PathBuf::from(&args[1]);
    let target = args.get(2).cloned().unwrap_or_else(|| "ovm".into());
    let out_dir = PathBuf::from(args.get(3).cloned().unwrap_or_else(|| "reports".into()));
    let dump = args.get(4).is_some_and(|flag| flag == "dump");

    let worker = std::thread::Builder::new()
        .stack_size(rusty_valkyrie::legion::cmds::LEGION_DRIVER_STACK_BYTES)
        .spawn(move || run(project, target, out_dir, dump))
        .expect("spawn worker");
    worker.join().expect("join worker");
}

fn run(project: PathBuf, target: String, out_dir: PathBuf, dump: bool) {
    let workspace = WorkspaceResolver::discover_for_project(&project).expect("discover workspace");
    let (passed, failed, skipped, entries) = run_tests_for_project(&workspace, &project, None, &[target.clone()], &[], false);

    let mut groups = BTreeMap::<(String, String), Occurrence>::new();
    let mut total_diagnostics = 0usize;
    let mut dumped = 0usize;
    for entry in entries.iter().filter(|entry| entry.error.is_some()) {
        let message = entry.error.clone().unwrap_or_default();
        if dump && dumped < 2 {
            println!("===== RAW ERROR SAMPLE {} ({}) =====", dumped, entry.name);
            println!("{message}");
            dumped += 1;
        }
        for diagnostic in parse_diagnostics(&message) {
            total_diagnostics += 1;
            let key = (diagnostic.id.clone(), diagnostic.symbol.clone());
            let slot = groups.entry(key).or_insert_with(|| Occurrence {
                id: diagnostic.id.clone(),
                symbol: diagnostic.symbol.clone(),
                first_file: diagnostic.file.clone(),
                first_line: diagnostic.line,
                first_entry: entry.name.clone(),
                sample: diagnostic.text.chars().take(240).collect(),
                ..Occurrence::default()
            });
            slot.count += 1;
        }
    }

    let mut unique = groups.into_values().collect::<Vec<_>>();
    unique.sort_by(|left, right| right.count.cmp(&left.count).then(left.id.cmp(&right.id)).then(left.symbol.cmp(&right.symbol)));

    fs::create_dir_all(&out_dir).expect("create report dir");
    let stem = out_dir.join("std-diagnostic-baseline-2026-11-04");
    let mut json = String::new();
    json.push_str("{\n");
    json.push_str(&format!("  \"tool\": \"b_diagnostic_snapshot\",\n"));
    json.push_str(&format!("  \"generated_command\": \"cargo +nightly-2026-10-23 run -q --manifest-path projects/nyar.rs/Cargo.toml -p rusty-valkyrie --example b_diagnostic_snapshot -- <project_dir> <target> <out_dir>\",\n"));
    json.push_str(&format!("  \"project\": \"{}\",\n", json_escape(&project.to_string_lossy())));
    json.push_str(&format!("  \"target\": \"{}\",\n", json_escape(&target)));
    json.push_str(&format!(
        "  \"totals\": {{\"passed\": {passed}, \"failed\": {failed}, \"skipped\": {skipped}, \"entries\": {}, \"diagnostic_occurrences\": {total_diagnostics}, \"unique_keys\": {}}},\n",
        entries.len(),
        unique.len()
    ));
    json.push_str("  \"groups\": [\n");
    for (index, group) in unique.iter().enumerate() {
        json.push_str(&format!(
            "    {{\"id\": \"{}\", \"symbol\": \"{}\", \"count\": {}, \"first_file\": \"{}\", \"first_line\": {}, \"first_entry\": \"{}\", \"attribution_guess\": \"{}\", \"sample\": \"{}\"}}{}\n",
            json_escape(&group.id),
            json_escape(&group.symbol),
            group.count,
            json_escape(&group.first_file),
            group.first_line,
            json_escape(&group.first_entry),
            attribution_guess(&Diagnostic { id: group.id.clone(), symbol: group.symbol.clone(), file: group.first_file.clone(), line: group.first_line, text: String::new() }),
            json_escape(&group.sample),
            if index + 1 == unique.len() { "" } else { "," }
        ));
    }
    json.push_str("  ]\n}\n");
    fs::write(stem.with_extension("json"), json).expect("write json snapshot");

    let mut markdown = String::new();
    markdown.push_str("# std 诊断基线快照（2026-11-04）\n\n");
    markdown.push_str(&format!(
        "- project: `{}`\n- target: `{}`\n- totals: passed={passed} failed={failed} skipped={skipped} entries={} diagnostic_occurrences={total_diagnostics} unique_keys={}\n",
        project.display(),
        target,
        entries.len(),
        unique.len()
    ));
    markdown.push_str("- key: (diagnostic id, 首出现符号)；`count` 为全部条目的诊断出现次数\n");
    markdown.push_str("- 重跑命令：\n\n```powershell\n$env:CARGO_TARGET_DIR='e:\\victory 胜利女神\\nyar-vm.rs\\.target-std-loop'\ncargo +nightly-2026-10-23 run -q --manifest-path 'e:\\victory 胜利女神\\nyar-vm.rs\\projects\\nyar.rs\\Cargo.toml' -p rusty-valkyrie --example b_diagnostic_snapshot -- 'e:\\victory 胜利女神\\valkyrie.v\\projects\\std' ovm 'e:\\victory 胜利女神\\reports'\n```\n\n");
    markdown.push_str("| # | count | id | symbol | first file:line | guess |\n|---|---|---|---|---|---|\n");
    for (index, group) in unique.iter().enumerate() {
        markdown.push_str(&format!(
            "| {} | {} | `{}` | `{}` | `{}:{}` | {} |\n",
            index + 1,
            group.count,
            group.id,
            group.symbol,
            group.first_file,
            group.first_line,
            attribution_guess(&Diagnostic { id: group.id.clone(), symbol: group.symbol.clone(), file: group.first_file.clone(), line: group.first_line, text: String::new() })
        ));
    }
    let mut by_id = BTreeMap::<&str, (usize, usize)>::new();
    let mut by_attribution = BTreeMap::<&str, (usize, usize)>::new();
    for group in &unique {
        let guess = attribution_guess(&Diagnostic { id: group.id.clone(), symbol: group.symbol.clone(), file: group.first_file.clone(), line: group.first_line, text: String::new() });
        by_id.entry(group.id.as_str()).or_insert((0, 0)).0 += 1;
        by_id.entry(group.id.as_str()).or_insert((0, 0)).1 += group.count;
        by_attribution.entry(guess).or_insert((0, 0)).0 += 1;
        by_attribution.entry(guess).or_insert((0, 0)).1 += group.count;
    }
    markdown.push_str("\n## 条数分布\n\n| id | 唯一键数 | 出现次数 |\n|---|---|---|\n");
    for (id, (keys, occurrences)) in &by_id {
        markdown.push_str(&format!("| `{id}` | {keys} | {occurrences} |\n"));
    }
    markdown.push_str("\n| 归属猜测 | 唯一键数 | 出现次数 |\n|---|---|---|\n");
    for (guess, (keys, occurrences)) in &by_attribution {
        markdown.push_str(&format!("| {guess} | {keys} | {occurrences} |\n"));
    }
    markdown.push_str("\n## 归属猜测口径\n\n- harness：`valkyrie.legion*` / workspace / planner 诊断。\n- std：首现位置在 `source/`（std 源码面）且不属于作用域/解析引擎类错误。\n- compiler：`valkyrie.hir.unresolved_call` / `no_method_candidate` / `ambiguous_call`，以及 `valkyrie.hir.*`、`valkyrie.mir.*`、`valkyrie.syntax.*`、`valkyrie.emitter.*`、`CAN*`、`SMIR*`、`CMIR*`。\n- unknown：未匹配上述规则。\n");
    fs::write(stem.with_extension("md"), markdown).expect("write markdown snapshot");

    println!("SNAPSHOT totals passed={passed} failed={failed} skipped={skipped} entries={}", entries.len());
    println!("SNAPSHOT diagnostic_occurrences={total_diagnostics} unique_keys={}", unique.len());
    for group in unique.iter().take(30) {
        println!("  x{} {} `{}` first={}:{} guess={}", group.count, group.id, group.symbol, group.first_file, group.first_line, attribution_guess(&Diagnostic { id: group.id.clone(), symbol: group.symbol.clone(), file: group.first_file.clone(), line: group.first_line, text: String::new() }));
    }
    println!("SNAPSHOT written: {} , {}", stem.with_extension("json").display(), stem.with_extension("md").display());
}
