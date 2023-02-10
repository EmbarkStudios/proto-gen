//! A Runner that extends proto-gen with a cli for code generation without direct build dependencies
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

mod kv;
use kv::KvValueParser;

use std::fmt::Debug;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use tempfile::TempDir;
use tonic_build::Builder;

use proto_gen::ProtoWorkspace;

const GENERATED_OUT_BASE_NAME: &str = "proto_types";

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
        #[command(subcommand)]
        strategy: Strategy,
    },
    /// Generate new Rust code for proto files, overwriting old files if present
    Generate {
        #[command(subcommand)]
        strategy: Strategy,
    },
}

#[derive(Subcommand, Debug)]
enum Strategy {
    /// Only target a specific workspace
    Workspace {
        /// Specifically generate files for this workspace
        #[clap(flatten)]
        workspace: WorkspaceOpts,
    },
    /// Recursively search from a base directory to find proto files.
    /// Running recursively forces some project assumption. Will only work with a project structure
    /// that has root/proto (for `*.proto` files, not nested), root/src (for code),
    /// will destructively create src/proto_types to place rust-code, and will create a tempdir for
    /// temporary files. The structure is validated, but assuming a valid structure, all data at
    /// /root/proto_types will be replaced if running `generate`
    Recursive {
        /// Start recursively searching from this directory
        #[clap(short, long)]
        base: PathBuf,
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

    for (k, v) in opts.tonic_opts.type_attributes.into_iter() {
        bldr = bldr.type_attribute(k, v);
    }

    for (k, v) in opts.tonic_opts.client_attributes.into_iter() {
        bldr = bldr.client_mod_attribute(k, v);
    }

    for (k, v) in opts.tonic_opts.server_attributes.into_iter() {
        bldr = bldr.server_mod_attribute(k, v);
    }

    let fmt = opts.format;
    let res = match opts.routine {
        Routine::Validate { strategy } => match strategy {
            Strategy::Workspace { workspace } => run_ws(workspace, bldr, false, fmt),
            Strategy::Recursive { base } => run_recursively(base, bldr, false, fmt),
        },
        Routine::Generate { strategy } => match strategy {
            Strategy::Workspace { workspace } => run_ws(workspace, bldr, true, fmt),
            Strategy::Recursive { base } => run_recursively(base, bldr, true, fmt),
        },
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
            ProtoWorkspace {
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
            ProtoWorkspace {
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

fn run_recursively(base: PathBuf, bldr: Builder, commit: bool, format: bool) -> Result<(), String> {
    let proto_dirs = find_proto_dirs(base)?;
    for dir in proto_dirs {
        proto_gen::run_proto_gen(
            ProtoWorkspace {
                proto_dir: dir.proto_dir,
                proto_files: dir.proto_files,
                tmp_dir: dir.tmp_dir.path().to_path_buf(),
                output_dir: dir.output_dir,
            },
            bldr.clone(),
            commit,
            format,
        )?;
    }
    Ok(())
}

#[derive(Debug)]
struct FoundWorkspace {
    proto_dir: PathBuf,
    proto_files: Vec<PathBuf>,
    tmp_dir: TempDir,
    output_dir: PathBuf,
}

fn find_proto_dirs(base: impl AsRef<Path> + Debug) -> Result<Vec<FoundWorkspace>, String> {
    let rd = fs::read_dir(&base)
        .map_err(|e| format!("Failed to read base path to search for protobufs {base:?} {e}"))?;
    // Since we need to validate this later, and protobuf files are merged by package because
    // of course they are, we need to iterate in order......... SCREAM
    let mut sub_dirs = vec![];
    for dir in rd {
        let d = dir.map_err(|e| {
            format!("Failed to read DirEntry while recursively looking for proto files {e}")
        })?;
        sub_dirs.push(d.path());
    }
    sub_dirs.sort();
    let mut protodirs = vec![];
    for path in sub_dirs {
        let path_str = path
            .to_str()
            .ok_or_else(|| format!("Found non-utf8 path {path:?} while searching for protos"))?;
        let metadata = path
            .metadata()
            .map_err(|e| format!("Failed to get metadata for path {path:?} {e}"))?;
        if metadata.is_dir() {
            // Recursively find directory containing proto files
            protodirs.extend(find_proto_dirs(path.clone())?);
        } else if path_str.ends_with(".proto") {
            // Found a proto containing dir
            let proto_dir = path.parent().ok_or_else(|| format!("Stepped back up one directory without finding parent, nonsensical error, path {path:?}"))?;
            let mut proto_files = vec![];
            for sub in
                fs::read_dir(proto_dir).map_err(|e| format!("Failed to read proto dir {e}"))?
            {
                let sub = sub.map_err(|e| {
                    format!("Failed to get DirEntry while traversiong {path:?} {e}")
                })?;
                let file_name = sub.file_name();
                let file_name_str = file_name.to_str().ok_or_else(|| {
                    format!(
                        "Found entry in proto dir {:?} with a non-utf8 name {:?}",
                        proto_dir, file_name
                    )
                })?;
                if file_name_str.ends_with(".proto") {
                    proto_files.push(sub.path());
                }
            }
            proto_files.sort();
            let workspace_root = proto_dir
                .parent()
                .ok_or_else(|| format!("Found no parent for proto dir {proto_dir:?}"))?
                .to_path_buf();
            let tmp =
                tempfile::tempdir().map_err(|e| format!("Failed to create a temp dir {e}"))?;
            let src_root = workspace_root.join("src");
            let output_dir = src_root.join(GENERATED_OUT_BASE_NAME);
            protodirs.push(FoundWorkspace {
                proto_dir: proto_dir.to_path_buf(),
                proto_files,
                tmp_dir: tmp,
                output_dir,
            });
            // We're assuming no nesting to make things simpler
            break;
        }
    }
    protodirs.sort_by(|a, b| a.proto_dir.cmp(&b.proto_dir));
    Ok(protodirs)
}
