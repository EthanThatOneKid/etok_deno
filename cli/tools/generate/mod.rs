// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

// use deno_core::anyhow;
use deno_core::error::AnyError;
// use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
// use deno_runtime::colors;

use crate::args::FileFlags;
use crate::args::Flags;
use crate::args::GenerateFlags;
// use crate::args::TsConfigType;
// use crate::args::TypeCheckMode;
use crate::graph_util::create_graph_and_maybe_check;
// use crate::graph_util::error_for_any_npm_specifier;
use crate::proc_state::ProcState;
// use crate::util;
// use crate::util::display;
// use crate::util::file_watcher::ResolutionResult;

mod parse_comments;
mod quoted_split;

use parse_comments::{parse_comments, ParsedComment};

fn make_file_filter(files: FileFlags) -> impl FnMut(&str) -> bool {
  let mut filters = Vec::new();
  true
}

fn make_comment_filter(
  run: Option<regex::Regex>,
  skip: Option<regex::Regex>,
) -> impl FnMut(&ParsedComment) -> bool {
  if let (Some(run), Some(skip)) = (run, skip) {
    move |comment: &ParsedComment| {
      let original_comment = comment.original();
      run.is_match(&original_comment) && !skip.is_match(&original_comment)
    }
  } else if let Some(run) = run {
    move |comment: &ParsedComment| run.is_match(&comment.original())
  } else if let Some(skip) = skip {
    move |comment: &ParsedComment| !skip.is_match(&comment.original())
  } else {
    move |_: &ParsedComment| true
  }
}

pub async fn generate(
  flags: Flags,
  generate_flags: GenerateFlags,
) -> Result<(), AnyError> {
  let ps = ProcState::build(flags).await?;
  let entrypoint = resolve_url_or_path(&generate_flags.source_file)?;
  let graph = Arc::try_unwrap(
    create_graph_and_maybe_check(entrypoint.clone(), &ps).await?,
  )
  .unwrap();

  for module in graph.modules() {
    let source_file = module.specifier.to_string();
    if !file_filter(&source_file) {
      continue;
    }

    let source_code = Arc::try_unwrap(module.maybe_source.unwrap()).unwrap();
    let comments = parse_comments(source_code);
    for comment in comments {
      let args = comment.args();
      let alias = comment.alias();
      let line = comment.line();
      let character = comment.character();
      let original = comment.original();

      let mut args = args.iter();
      let command = args.next().unwrap();
      let args: Vec<&str> = args.map(|s| s.as_str()).collect();

      let mut command_args = vec![command.as_str()];
      command_args.extend(args.iter().map(|s| s.as_str()));

      let mut command = std::process::Command::new(command_args[0]);
      command.args(&command_args[1..]);

      let output = command.output().unwrap();
      let stdout = String::from_utf8(output.stdout).unwrap();
      let stderr = String::from_utf8(output.stderr).unwrap();

      println!("stdout: {}", stdout);
      println!("stderr: {}", stderr);
    }
  }

  Ok(())
}
