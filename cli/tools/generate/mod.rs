// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_core::error::AnyError;
use deno_core::resolve_url_or_path;
use deno_core::url::Url;
use glob::Pattern;
use std::collections::HashMap;
use std::env::consts::OS;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use crate::args::FileFlags;
use crate::args::Flags;
use crate::args::GenerateFlags;
use crate::graph_util::create_graph_and_maybe_check;
use crate::proc_state::ProcState;

mod parse_comments;
mod quoted_split;

use parse_comments::{parse_comments, ParsedComment};

// https://docs.rs/glob/latest/glob/struct.Pattern.html#method.matches_path_with

/// Runs the `deno generate` commands in the given module.
pub async fn generate(
  flags: Flags,
  generate_flags: GenerateFlags,
) -> Result<(), AnyError> {
  let file_filter =
    file_filter_from_file_flags(&generate_flags.files, |_| true);
  let source_file = resolve_url_or_path(&generate_flags.source_file)?;
  if !file_filter(path_from_url(&source_file)) {
    return Ok(());
  }

  let ps = ProcState::build(flags).await?;
  let graph =
    Arc::try_unwrap(create_graph_and_maybe_check(source_file, &ps).await?)
      .unwrap();
  let comment_filter =
    comment_filter_from_generate_flags(&generate_flags, |_| true);
  let verbose = generate_flags.verbose.unwrap_or(false);
  let dry_run = generate_flags.dry_run.unwrap_or(false);
  let trace = generate_flags.trace.unwrap_or(false);

  for module in graph.modules() {
    let module_specifier = &module.specifier;
    if !file_filter(path_from_url(module_specifier)) {
      continue;
    }

    let generate_commands =
      collect_generate_commands(module, &generate_flags, &comment_filter)?;

    for (parsed_comment, command) in generate_commands {
      if verbose {
        println!(
          "Running {} in <{}>",
          parsed_comment.command_full(),
          module_specifier,
        );
      }

      if dry_run {
        continue;
      }

      let output = command.output()?;
      if verbose || trace {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!("stdout: {}", stdout);
        println!("stderr: {}", stderr);
      }

      if trace {
        println!("exit status {}", output.status);
      }
    }
  }

  Ok(())
}

fn path_from_url(url: &Url) -> &'static Path {
  &url.to_file_path().unwrap().as_path()
}

/// Makes a filter function that filters out files that should not be
/// included in the graph.
fn file_filter_from_file_flags<'a, F>(
  file_flags: &'a FileFlags,
  filter_fn: F,
) -> impl Fn(&Path) -> bool + 'a
where
  F: Fn(&Path) -> bool + 'a,
{
  let include_patterns = file_flags
    .include
    .iter()
    .map(|path| Pattern::new(path.to_str().unwrap()).unwrap());
  let ignore_patterns = file_flags
    .ignore
    .iter()
    .map(|path| Pattern::new(path.to_str().unwrap()).unwrap())
    .collect::<Vec<Pattern>>();

  move |path: &Path| {
    if !filter_fn(path) {
      false
    } else if ignore_patterns
      .iter()
      .any(|pattern| pattern.matches(path.to_str().unwrap()))
    {
      false
    } else {
      include_patterns
        .clone()
        .filter(|pattern| pattern.matches(path.to_str().unwrap()))
        .next()
        .is_some()
    }
  }
}

/// Makes a filter function that filters out comments that should not be
/// included in the graph.
fn comment_filter_from_generate_flags<F>(
  generate_flags: &GenerateFlags,
  filter_fn: F,
) -> impl Fn(&ParsedComment) -> bool
where
  F: Fn(&str) -> bool,
{
  let run_regex = generate_flags
    .run
    .as_ref()
    .map(|run| regex::Regex::new(run).unwrap());
  let skip_regex = generate_flags
    .skip
    .as_ref()
    .map(|skip| regex::Regex::new(skip).unwrap());

  move |comment: &ParsedComment| {
    let should_include = match (run_regex.as_ref(), skip_regex.as_ref()) {
      (Some(run), Some(skip)) => {
        run.is_match(&comment.original) && !skip.is_match(&comment.original)
      }
      (Some(run), None) => run.is_match(&comment.original),
      (None, Some(skip)) => !skip.is_match(&comment.original),
      (None, None) => true,
    };
    should_include && filter_fn(comment.original.as_str())
  }
}

/// Collects and runs the generate commands from the comments in the given module.
fn collect_generate_commands<'a>(
  module: &'a deno_graph::Module,
  generate_flags: &'a GenerateFlags,
  filter_fn: &'a dyn Fn(&ParsedComment) -> bool,
) -> Result<Vec<(ParsedComment, &'a mut std::process::Command)>, AnyError> {
  let source_code = Arc::get_ref(&module.maybe_source.unwrap()).unwrap();
  let comments = parse_comments(source_code);
  let mut aliases: HashMap<&str, &ParsedComment> = HashMap::new();
  let mut commands: Vec<&mut std::process::Command> = Vec::new();
  for comment in comments {
    if let Some(alias) = comment.alias() {
      aliases.insert(alias, &comment);
      continue;
    }

    if let Some(filter_fn) = filter_fn {
      if !filter_fn(&comment) {
        continue;
      }
    }

    let (cmd, cmd_args) = match aliases.get(comment.command()) {
      Some(alias) => {
        let mut args = alias.args().to_vec();
        args.extend(comment.args());
        (alias.command(), args)
      }
      None => (comment.command(), comment.args()),
    };

    let mut command = Command::new(cmd);
    command.args(cmd_args).envs(envs_from(module, &comment));
    commands.push((comment, command));
  }

  Ok(commands)
}

/// Returns the environment variables to be passed to the command.
fn envs_from<'a>(
  module: &'a deno_graph::Module,
  comment: &'a ParsedComment,
) -> Vec<(&'a str, &'a str)> {
  let deno_dir = module
    .specifier
    .to_file_path()
    .expect("Module specifier is not a file path")
    .parent()
    .expect("Module path does not have a parent directory")
    .to_str()
    .expect("Parent directory is not a valid UTF-8 string");

  vec![
    ("DENO_OS", OS),
    ("DENO_MODULE", &module.specifier),
    ("DENO_LINE", comment.line),
    ("DENO_CHARACTER", comment.character)("DENO_DIR", deno_dir),
    ("DOLLAR", "$"),
  ]
}
