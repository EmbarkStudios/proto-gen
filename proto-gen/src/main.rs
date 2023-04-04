//! A Runner that extends proto-gen with a cli for code generation without direct build dependencies
#![warn(clippy::pedantic)]

mod gen;
mod kv;

use kv::KvValueParser;

use std::fmt::Debug;
use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use tonic_build::Builder;

use gen::ProtoWorkspace;

/// A simple runner that generates and moved rust-files form protos tonic-build into a workspace.
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

#[derive(Args, Debug, Clone)]
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

#[derive(Debug, Args, Clone)]
struct WorkspaceOpts {
    /// Directories containing proto files to source (Ex. Dependencies),
    /// needs to include any directory containing files to be included in generation.
    #[clap(short = 'd', long)]
    proto_dirs: Vec<PathBuf>,

    /// The files to be included in generation.
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
    run_with_opts(opts)
}

fn run_with_opts(opts: Opts) -> Result<(), i32> {
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
        gen::run_generation(
            &ProtoWorkspace {
                proto_dirs: opts.proto_dirs,
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
        gen::run_generation(
            &ProtoWorkspace {
                proto_dirs: opts.proto_dirs,
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

#[cfg(all(test, feature = "protoc-tests"))]
mod tests {
    use super::*;
    use std::io::ErrorKind;
    use tempfile::TempDir;

    struct SimpleTestCfg {
        _keep_alive_project_base: TempDir,
        tonic_opts: TonicOpts,
        workspace: WorkspaceOpts,
    }

    fn create_simple_test_cfg(tmp_dir: Option<PathBuf>) -> SimpleTestCfg {
        let project_base = tempfile::tempdir().unwrap();
        let src = project_base.path().join("src");
        let proto_files_dir = project_base.path().join("proto");
        let my_proto = proto_files_dir.join("my-proto.proto");
        let ex_proto_content = r#"syntax = "proto3";

package my_proto;

message MyNestedMessage {
  int32 some_field = 1;
}

// My comment
message TestMessage {
  // My field comment!
  int32 field_one = 1;
  string field_two = 2;
  MyNestedMessage my_very_long_field_hopefully_we_can_get_a_format_trigger_off_this_bad_boi = 3;
}"#;
        std::fs::create_dir_all(&proto_files_dir).unwrap();
        std::fs::write(&my_proto, ex_proto_content).unwrap();
        let proto_types_dir = src.join("proto_types");
        let tonic_opts = TonicOpts {
            build_server: false,
            build_client: false,
            generate_transport: false,
            type_attributes: vec![],
            client_attributes: vec![],
            server_attributes: vec![],
        };
        let workspace = WorkspaceOpts {
            proto_dirs: vec![proto_files_dir],
            proto_files: vec![my_proto],
            tmp_dir,
            output_dir: proto_types_dir.clone(),
        };
        SimpleTestCfg {
            _keep_alive_project_base: project_base,
            tonic_opts,
            workspace,
        }
    }

    #[test]
    fn full_generate_single_file_project() {
        let test_cfg = create_simple_test_cfg(None);
        let opts = Opts {
            tonic_opts: test_cfg.tonic_opts.clone(),
            format: true,
            routine: Routine::Generate {
                workspace: test_cfg.workspace.clone(),
            },
        };
        // Generate
        run_with_opts(opts).unwrap();
        let opts = Opts {
            tonic_opts: test_cfg.tonic_opts.clone(),
            format: true,
            routine: Routine::Validate {
                workspace: test_cfg.workspace.clone(),
            },
        };
        // Validate it's the same after generation
        run_with_opts(opts).unwrap();
        let opts = Opts {
            tonic_opts: test_cfg.tonic_opts.clone(),
            format: false,
            routine: Routine::Validate {
                workspace: test_cfg.workspace.clone(),
            },
        };
        // Validate it's not the same if specifying no fmt
        match run_with_opts(opts) {
            Ok(_) => panic!("Expected fail on diff"),
            Err(code) => {
                assert_eq!(1, code);
            }
        }
    }

    #[test]
    fn full_generate_single_file_project_does_not_remove_explicit_temp() {
        let my_output_tmp = tempfile::tempdir().unwrap();
        let test_cfg = create_simple_test_cfg(Some(my_output_tmp.path().to_path_buf()));
        let opts = Opts {
            tonic_opts: test_cfg.tonic_opts.clone(),
            format: false,
            routine: Routine::Generate {
                workspace: test_cfg.workspace.clone(),
            },
        };
        // Generate
        run_with_opts(opts).unwrap();
        if let Err(e) = std::fs::metadata(my_output_tmp.path()) {
            if e.kind() == ErrorKind::NotFound {
                eprintln!("Dir deleted!");
            }
        }
        if let Err(e) = std::fs::metadata(my_output_tmp.path().join("my-proto.rs")) {
            if e.kind() == ErrorKind::NotFound {
                eprintln!("File not found!");
            }
        }
        assert_exists_not_empty(&my_output_tmp.path().join("my_proto.rs"));
    }

    #[test]
    fn full_generate_nested_project() {
        let project_base = tempfile::tempdir().unwrap();
        let src = project_base.path().join("src");
        let proto_files_dir = project_base.path().join("proto");
        let my_proto = proto_files_dir.join("my-proto.proto");
        let ex_proto_content = r#"syntax = "proto3";

package my_proto;

import "imports/dependency.proto";
import "imports/nested/nested_one.proto";

message MyNestedMessage {
  int32 some_field = 1;
  imports.dependency.Dependency dependency = 2;
  imports.nested.NestedOne nested_one = 3;
}

// My comment
message TestMessage {
  // My field comment!
  int32 field_one = 1;
  string field_two = 2;
  MyNestedMessage my_very_long_field_hopefully_we_can_get_a_format_trigger_off_this_bad_boi = 3;
}"#;
        std::fs::create_dir_all(&proto_files_dir).unwrap();
        std::fs::write(&my_proto, ex_proto_content).unwrap();
        let dep_dir = proto_files_dir.join("imports");
        std::fs::create_dir_all(&dep_dir).unwrap();
        let first_dep_proto = r#"syntax = "proto3";
package imports.dependency;

import "imports/nested/nested_transitive.proto";

message Dependency {
  int64 my_dep_field = 1;
  imports.nested.NestedTransitiveMsg ntm = 2;
}
"#;
        std::fs::write(dep_dir.join("dependency.proto"), first_dep_proto).unwrap();
        let nested_dep_proto_dir = dep_dir.join("nested");
        let nested_first = r#"syntax = "proto3";
package imports.nested;

message NestedOne {
  int32 my_field_of_first_nested = 1;
}
"#;
        std::fs::create_dir_all(&nested_dep_proto_dir).unwrap();
        std::fs::write(nested_dep_proto_dir.join("nested_one.proto"), nested_first).unwrap();
        let nested_trns = r#"syntax = "proto3";
package imports.nested;

message NestedTransitiveMsg {
  int32 my_transitive_nested_field = 1;
}
"#;
        std::fs::write(
            nested_dep_proto_dir.join("nested_transitive.proto"),
            nested_trns,
        )
        .unwrap();
        let proto_types_dir = src.join("proto_types");
        let tonic_opts = TonicOpts {
            build_server: false,
            build_client: false,
            generate_transport: false,
            type_attributes: vec![],
            client_attributes: vec![],
            server_attributes: vec![],
        };
        let workspace = WorkspaceOpts {
            proto_dirs: vec![proto_files_dir, dep_dir, nested_dep_proto_dir],
            proto_files: vec![my_proto],
            tmp_dir: None,
            output_dir: proto_types_dir.clone(),
        };
        let opts = Opts {
            tonic_opts,
            format: false,
            routine: Routine::Generate { workspace },
        };
        run_with_opts(opts).unwrap();
        assert_exists_not_empty(&proto_types_dir.join("my_proto.rs"));
        assert_exists_not_empty(&proto_types_dir.join("imports.rs"));
        assert_exists_not_empty(&proto_types_dir.join("imports").join("dependency.rs"));
        assert_exists_not_empty(&proto_types_dir.join("imports").join("nested.rs"));
    }
    fn assert_exists_not_empty(path: &Path) {
        let content = std::fs::read(path)
            .map_err(|e| format!("Failed to read {path:?}: {e}"))
            .unwrap();
        assert!(!content.is_empty(), "Empty file at {path:?}");
    }
}
