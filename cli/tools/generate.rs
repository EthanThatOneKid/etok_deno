// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Arc;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_runtime::colors;

use crate::args::CliOptions;
use crate::args::Flags;
use crate::args::GenerateFlags;
use crate::args::TsConfigType;
use crate::args::TypeCheckMode;
use crate::graph_util::create_graph_and_maybe_check;
use crate::graph_util::error_for_any_npm_specifier;
use crate::proc_state::ProcState;
use crate::util;
use crate::util::display;
use crate::util::file_watcher::ResolutionResult;

pub async fn generate(
  flags: Flags,
  generate_flags: GenerateFlags,
) -> Result<(), AnyError> {
  let cli_options = CliOptions::from_flags(flags)?;
  let source_file =
    Arc::new(cli_options.argv().get(0).unwrap().to_string()).as_ref();
  let module_specifier = resolve_url_or_path(source_file)?;
  let ps = ProcState::from_options(Arc::new(cli_options)).await?;
  let graph = create_graph_and_maybe_check(module_specifier, &ps).await?;

  let lines = util::fs::read_file_to_string(&source_file)
    .await?
    .lines()
    .filter(|l| l.starts_with("//deno:generate"));

  for line in lines {
    let command = line.trim_start_matches("//deno:generate").trim();
    let output = util::run_command(command)
      .await
      .map_err(|e| anyhow!("Failed to run command: {}", e))?;
    println!("{}", output);
  }

  Ok(())
}

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  ps: &ProcState,
) -> Result<deno_emit::BundleEmit, AnyError> {
  log::info!("{} {}", colors::green("Bundle"), graph.roots[0]);

  let ts_config_result = ps
    .options
    .resolve_ts_config_for_emit(TsConfigType::Bundle)?;
  if ps.options.type_check_mode() == TypeCheckMode::None {
    if let Some(ignored_options) = ts_config_result.maybe_ignored_options {
      log::warn!("{}", ignored_options);
    }
  }

  deno_emit::bundle_graph(
    graph,
    deno_emit::BundleOptions {
      bundle_type: deno_emit::BundleType::Module,
      emit_options: ts_config_result.ts_config.into(),
      emit_ignore_directives: true,
    },
  )
}
