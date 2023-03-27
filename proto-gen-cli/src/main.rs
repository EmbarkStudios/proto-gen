//! A Runner that extends proto-gen with a cli for code generation without direct build dependencies
#![warn(clippy::pedantic)]
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

mod kv;
use kv::KvValueParser;

use std::fmt::Debug;
use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use tonic_build::Builder;

use proto_gen::ProtoWorkspace;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Opts {
    #[clap(flatten)]
    tonic_opts: TonicOpts,
    /// Use `rustfmt` on the code after generation, `rustfmt` needs to be on the path
    #[clap(short, long)]
    format: bool,
    #[command(subcommand)]
    routine: Routine,
}

#[derive(Args, Debug)]
struct TonicOpts {
    /// Whether to build server code
    #[clap(short = 's', long)]
    build_server: bool,
    /// Whether to build client code
    #[clap(short = 'c', long)]
    build_client: bool,

    /// Whether to generate the ::connect and similar functions for tonic.
    #[clap(long)]
    generate_transport: bool,

    /// Type attributes to add.
    #[clap(long = "type-attribute", value_parser=KvValueParser)]
    type_attributes: Vec<(String, String)>,

    /// Client mod attributes to add.
    #[clap(long = "client-attribute", value_parser=KvValueParser)]
    client_attributes: Vec<(String, String)>,

    /// Server mod attributes to add.
    #[clap(long = "server-attribute", value_parser=KvValueParser)]
    server_attributes: Vec<(String, String)>,
}

#[derive(Subcommand, Debug)]
enum Routine {
    /// Generate new Rust code for proto files, checking current files for differences.
    /// Returns error code 1 on any found differences.
    Validate {
        #[clap(flatten)]
        workspace: WorkspaceOpts,
    },
    /// Generate new Rust code for proto files, overwriting old files if present
    Generate {
        #[clap(flatten)]
        workspace: WorkspaceOpts,
    },
}

#[derive(Debug, Args)]
struct WorkspaceOpts {
    /// The directory containing proto files
    #[clap(short = 'd', long)]
    proto_dir: PathBuf,
    /// The files to be included in generation
    #[clap(short = 'f', long)]
    proto_files: Vec<PathBuf>,
    /// Temporary working directory, if left blank, `tempfile` is used to create a temporary
    /// directory.
    #[clap(short, long)]
    tmp_dir: Option<PathBuf>,
    /// Where to place output files. Will get cleaned up (all contents deleted)
    /// A module file will be placed in the parent of this directory.
    #[clap(short, long)]
    output_dir: PathBuf,
}

fn main() -> Result<(), i32> {
    let opts: Opts = Opts::parse();
    let mut bldr = tonic_build::configure()
        .build_client(opts.tonic_opts.build_client)
        .build_server(opts.tonic_opts.build_server)
        .build_transport(opts.tonic_opts.generate_transport);

    for (k, v) in opts.tonic_opts.type_attributes {
        bldr = bldr.type_attribute(k, v);
    }

    for (k, v) in opts.tonic_opts.client_attributes {
        bldr = bldr.client_mod_attribute(k, v);
    }

    for (k, v) in opts.tonic_opts.server_attributes {
        bldr = bldr.server_mod_attribute(k, v);
    }

    let fmt = opts.format;
    let res = match opts.routine {
        Routine::Validate { workspace } => run_ws(workspace, bldr, false, fmt),
        Routine::Generate { workspace } => run_ws(workspace, bldr, true, fmt),
    };
    if let Err(err) = res {
        eprintln!("Failed to run command, E: {err}");
        return Err(1);
    }
    Ok(())
}

fn run_ws(opts: WorkspaceOpts, bldr: Builder, commit: bool, format: bool) -> Result<(), String> {
    if opts.proto_files.is_empty() {
        return Err("--proto-files needs at least one file to generate".to_string());
    }
    if let Some(tmp) = opts.tmp_dir {
        proto_gen::run_proto_gen(
            &ProtoWorkspace {
                proto_dir: opts.proto_dir,
                proto_files: opts.proto_files,
                tmp_dir: tmp,
                output_dir: opts.output_dir,
            },
            bldr,
            commit,
            format,
        )
    } else {
        // Deleted on drop
        let tmp = tempfile::tempdir().map_err(|e| format!("Failed to create tempdir {e}"))?;
        proto_gen::run_proto_gen(
            &ProtoWorkspace {
                proto_dir: opts.proto_dir,
                proto_files: opts.proto_files,
                tmp_dir: tmp.path().to_path_buf(),
                output_dir: opts.output_dir,
            },
            bldr,
            commit,
            format,
        )
    }
}
