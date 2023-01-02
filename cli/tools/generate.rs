// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use std::path::PathBuf;
use std::sync::Arc;

use deno_core::error::AnyError;
use deno_core::futures::FutureExt;
use deno_core::resolve_url_or_path;
use deno_runtime::colors;

use crate::args::BundleFlags;
use crate::args::CliOptions;
use crate::args::Flags;
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
  bundle_flags: BundleFlags,
) -> Result<(), AnyError> {
  let cli_options = Arc::new(CliOptions::from_flags(flags)?);
  let resolver = |_| {
    let cli_options = cli_options.clone();
    let source_file1 = bundle_flags.source_file.clone();
    let source_file2 = bundle_flags.source_file.clone();
    async move {
      let module_specifier = resolve_url_or_path(&source_file1)?;

      log::debug!(">>>>> generate START");
      let ps = ProcState::from_options(cli_options).await?;
      let graph = create_graph_and_maybe_check(module_specifier, &ps).await?;

      let mut paths_to_watch: Vec<PathBuf> = graph
        .specifiers()
        .filter_map(|(_, r)| {
          r.as_ref().ok().and_then(|(s, _, _)| s.to_file_path().ok())
        })
        .collect();

      if let Ok(Some(import_map_path)) = ps
        .options
        .resolve_import_map_specifier()
        .map(|ms| ms.and_then(|ref s| s.to_file_path().ok()))
      {
        paths_to_watch.push(import_map_path);
      }

      Ok((paths_to_watch, graph, ps))
    }
    .map(move |result| match result {
      Ok((paths_to_watch, graph, ps)) => ResolutionResult::Restart {
        paths_to_watch,
        result: Ok((ps, graph)),
      },
      Err(e) => ResolutionResult::Restart {
        paths_to_watch: vec![PathBuf::from(source_file2)],
        result: Err(e),
      },
    })
  };

  let operation = |(ps, graph): (ProcState, Arc<deno_graph::ModuleGraph>)| {
    let out_file =bundle_flags.out_file.clone();
    async move {
      // at the moment, we don't support npm specifiers in deno generate, so show an error
      error_for_any_npm_specifier(&graph)?;

      for specifier in graph.specifiers() {
        // print the file specifier being traversed
        println!("{}", specifier.0);
      }

      log::debug!(">>>>> generate END");

      Ok(())
    }
  };

  let ts_config = if let TsConfigType::Auto = bundle_flags.ts_config {
    let module_specifier = resolve_url_or_path(&bundle_flags.source_file)?;
    util::ts_config::load_ts_config_from_root_dir(module_specifier, &cli_options).await
  } else {
    None
  };

  let result = util::file_watcher::run_resolution_loop(
    &resolver,
    &operation,
    flags.watch,
    ts_config.as_ref(),
  )
  .await;

  match result {
    Ok(()) => Ok(()),
    Err(e) => {
      log::debug!(">>>>> generate END with error");
      Err(e)
    }
  }
}

fn bundle_module_graph(
  graph: &deno_graph::ModuleGraph,
  _ps: &ProcState,
) -> Result<util::bundle::BundleOutput, AnyError> {
  let output_code =
    deno_bundler::bundle_source_code(graph.as_ref())?.source_code;
  let maybe_bundle_map = deno_bundler::create_bundle_map(graph.as_ref())?;
  Ok(util::bundle::BundleOutput {
    code: output_code,
    maybe_bundle_map,
  })
}

