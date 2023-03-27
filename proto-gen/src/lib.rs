//! A library that generates Rust code using tonic-build and places that code in a supplied directory
#![warn(clippy::pedantic)]
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fmt::{Debug, Write};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use tonic_build::Builder;

/// Generate protos for the provided proto workspace
/// # Errors
/// Miscellaneous errors accessing the filesystem (such as permissions),
/// and errors coming from `protoc`
pub fn run_proto_gen(
    proto_ws: &ProtoWorkspace,
    opts: Builder,
    commit: bool,
    format: bool,
) -> Result<(), String> {
    let top_mod_content = generate_to_tmp(proto_ws, opts).map_err(|e| {
        format!("Failed to generate prots into temp dir for proto workspace {proto_ws:?} {e}")
    })?;
    let old = &proto_ws.output_dir;
    let new = &proto_ws.tmp_dir;
    if format {
        recurse_fmt(new)?;
    }
    let diff = run_diff(old, new, &top_mod_content)?;
    if diff > 0 {
        println!("Found diff in {diff} protos at {:?}", proto_ws.output_dir);
        if commit {
            println!("Writing {diff} protos to {:?}", proto_ws.output_dir);
            recurse_copy_clean(new, old)?;
            let out_top_name = as_file_name_string(old)?;
            let out_parent = old.parent().ok_or_else(|| {
                format!("Failed to find parent for output dir {old:?} to place mod file")
            })?;
            let mod_file = out_parent.join(format!("{out_top_name}.rs"));
            fs::write(&mod_file, top_mod_content.as_bytes())
                .map_err(|e| format!("Failed to write parent module file to {mod_file:?} {e}"))?;
        } else {
            return Err(format!("Found {diff} diffs at {:?}", proto_ws.output_dir));
        }
    } else {
        println!("Found no diff at {:?}", proto_ws.output_dir);
    }
    Ok(())
}

#[derive(Debug)]
pub struct ProtoWorkspace {
    pub proto_dirs: Vec<PathBuf>,
    pub proto_files: Vec<PathBuf>,
    pub tmp_dir: PathBuf,
    pub output_dir: PathBuf,
}

#[inline]
fn gen_proto(
    src_dirs: &[impl AsRef<Path> + Debug],
    src_files: &[impl AsRef<Path>],
    out_dir: impl AsRef<OsStr>,
    opts: Builder,
) -> Result<(), String> {
    let old_out = std::env::var("OUT_DIR");
    std::env::set_var("OUT_DIR", out_dir);
    // Would by nice if we could just get a byte buffer instead of magic env write
    opts.compile(src_files, src_dirs)
        .map_err(|e| format!("Failed to compile protos from {src_dirs:?} {e}"))?;
    // Restore the env, cause why not
    if let Ok(old) = old_out {
        std::env::set_var("OUT_DIR", old);
    } else {
        std::env::remove_var("OUT_DIR");
    }
    Ok(())
}

fn generate_to_tmp(workspace: &ProtoWorkspace, opts: Builder) -> Result<String, String> {
    gen_proto(
        &workspace.proto_dirs,
        &workspace.proto_files,
        &workspace.tmp_dir,
        opts,
    )?;
    clean_up_file_structure(&workspace.tmp_dir)
}

fn clean_up_file_structure(out_dir: &Path) -> Result<String, String> {
    let rd = fs::read_dir(out_dir)
        .map_err(|e| format!("Failed read output dir {out_dir:?} when cleaning up files {e}"))?;
    let mut out_modules = ModuleContainer::Parent {
        name: "dummy".to_string(),
        location: out_dir.to_path_buf(),
        children: HashMap::new(),
    };
    for entry in rd {
        let entry = entry.map_err(|e| {
            format!(
                "Failed to read DirEntry when cleaning up output dir {:?} {e}",
                &out_dir
            )
        })?;
        let file_path = entry.path();
        let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata for entity {file_path:?} in output dir {out_dir:?} when cleaning up files {e}"))?;
        if metadata.is_file() {
            // Tonic build 0.7 generates a bunch of empty files for some reason, fixed in 0.8
            let content = fs::read(&file_path)
                .map_err(|e| format!("Failed to read generated file at path {file_path:?} {e}"))?;
            if content.is_empty() {
                fs::remove_file(&file_path).map_err(|e| {
                    format!("Failed to delete empty file {file_path:?} from temp directory {e}")
                })?;
            } else {
                out_modules.push_file(out_dir, &file_path)?;
            }
        }
    }
    let ModuleContainer::Parent { children, .. } = out_modules else {
        return Err("Top level module container is not a parent".to_string());
    };
    let mut sortable_children = children.into_values().collect::<Vec<ModuleContainer>>();
    // Linting, guh
    let mut top_level_mod = "#![allow(clippy::doc_markdown, clippy::use_self)]\n".to_string();
    sortable_children.sort_by(|a, b| a.get_name().cmp(b.get_name()));
    for module in sortable_children {
        module.dump_to_disk()?;
        let _ = top_level_mod.write_fmt(format_args!("pub mod {};\n", module.get_name()));
    }
    Ok(top_level_mod)
}

#[derive(Debug)]
enum ModuleContainer {
    Parent {
        name: String,
        location: PathBuf,
        children: HashMap<String, ModuleContainer>,
    },
    Node {
        name: String,
        location: PathBuf,
        file: PathBuf,
    },
}

impl ModuleContainer {
    fn push_file(&mut self, top_level: &Path, path: &Path) -> Result<(), String> {
        let file_path = path;
        let file_name = file_path
            .file_name()
            .ok_or_else(|| format!("Failed to get file name of path {file_path:?}"))?;
        let file_path_str = file_name
            .to_str()
            .ok_or_else(|| format!("Failed to convert path {file_name:?} to str"))?;
        let (nest, _rs) = file_path_str
            .rsplit_once('.')
            .ok_or_else(|| format!("File path string {file_path_str} is not valid utf8"))?;
        self.push_recurse(top_level, path, nest)?;
        Ok(())
    }

    fn push_recurse(
        &mut self,
        parent: &Path,
        path: impl AsRef<Path>,
        raw_name: &str,
    ) -> Result<(), String> {
        if let Some((cur, rest)) = raw_name.split_once('.') {
            match self {
                ModuleContainer::Parent { children, .. } => {
                    let new_parent = parent.join(cur);
                    if let Some(child) = children.get_mut(cur) {
                        child.push_recurse(&new_parent, path, rest)?;
                    } else {
                        let mut md = ModuleContainer::Parent {
                            name: cur.to_string(),
                            location: parent.to_path_buf(),
                            children: HashMap::new(),
                        };
                        md.push_recurse(&new_parent, path, rest)?;
                        children.insert(cur.to_string(), md);
                    }
                }
                ModuleContainer::Node { .. } => {
                    return Err(format!(
                        "Tried to push a child on a node {:?}",
                        path.as_ref()
                    ));
                }
            }
        } else {
            let ModuleContainer::Parent { children, .. } = self else {
                return Err(format!("Raw name {raw_name} did not belong to a parent node"));
            };
            children.insert(
                raw_name.to_string(),
                ModuleContainer::Node {
                    name: raw_name.to_string(),
                    location: parent.to_path_buf(),
                    file: path.as_ref().to_path_buf(),
                },
            );
        }
        Ok(())
    }

    fn dump_to_disk(&self) -> Result<(), String> {
        match self {
            ModuleContainer::Parent {
                name,
                children,
                location,
            } => {
                let dir = location.join(name);
                fs::create_dir_all(&dir)
                    .map_err(|e| format!("Failed to create module directory for {dir:?} {e}"))?;
                let mut sortable_children = children.values().collect::<Vec<&ModuleContainer>>();
                sortable_children.sort_by(|a, b| {
                    let a_name = a.get_name();
                    let b_name = b.get_name();
                    a_name.cmp(b_name)
                });
                let mut output = String::new();
                for sorted_child in sortable_children {
                    let _ = output.write_fmt(format_args!("pub mod {};", sorted_child.get_name()));
                    sorted_child.dump_to_disk()?;
                }
                let mod_file_location = location.join(format!("{name}.rs"));
                fs::write(&mod_file_location, output.as_bytes()).map_err(|e| {
                    format!("Failed to write module file at {mod_file_location:?} {e}")
                })?;
                Ok(())
            }
            ModuleContainer::Node {
                name,
                location,
                file,
            } => {
                let file_location = location.join(format!("{name}.rs"));
                if &file_location == file {
                    return Ok(());
                }
                fs::copy(file, &file_location).map_err(|e| {
                    format!("Failed to copy module file from {file:?} to {file_location:?} {e}")
                })?;
                fs::remove_file(file)
                    .map_err(|e| format!("Failed to remove original file from {file:?} {e}"))?;
                Ok(())
            }
        }
    }

    fn get_name(&self) -> &str {
        match self {
            ModuleContainer::Parent { name, .. } | ModuleContainer::Node { name, .. } => {
                name.as_str()
            }
        }
    }
}

fn as_file_name_string(path: impl AsRef<Path>) -> Result<String, String> {
    let path = path.as_ref();
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("Failed to get file_name of path {path:?}"))?;
    let file_name_str = file_name
        .to_str()
        .ok_or_else(|| format!("Failed to convert file_name {file_name:?} to utf8"))?;
    Ok(file_name_str.to_string())
}

fn run_diff(
    orig: impl AsRef<Path> + Debug,
    new: impl AsRef<Path> + Debug,
    new_mod: &str,
) -> Result<usize, String> {
    let orig_root = orig.as_ref();
    let orig_root_file_name = orig_root
        .file_name()
        .ok_or_else(|| format!("Failed to get filename when diffing original path {orig:?}"))?;
    let orig_root_file = orig_root_file_name.to_str()
        .ok_or_else(|| format!("Failed to convert filename {orig_root_file_name:?} when diffing original path {orig:?}"))?;
    let mut orig_files = collect_files(&orig, orig_root_file)?;
    let new_root = new.as_ref();
    let new_root_file_name = new_root
        .file_name()
        .ok_or_else(|| format!("Failed to get filename when diffing new path {new:?}"))?;
    let new_root_file = new_root_file_name.to_str()
        .ok_or_else(|| format!("Failed to convert filename {new_root_file_name:?} to utf8 when diffing new path {new:?}"))?;
    let new_files = collect_files(&new, new_root_file)?;
    let mut diff = 0;
    for file in &new_files {
        if vec_remove(file, &mut orig_files) {
            let orig_path = orig.as_ref().join(file);
            let new_path = new.as_ref().join(file);
            let a = fs::read(&orig_path)
                .map_err(|e| format!("Failed to read file at {orig_path:?} {e}"))?;
            let b = fs::read(&new_path)
                .map_err(|e| format!("Failed to read file at {new_path:?} {e}"))?;
            if a != b {
                eprintln!("Found diff in {file:?}");
                diff += 1;
            }
        } else {
            eprintln!("Found new proto at {file:?}");
            diff += 1;
        }
    }
    let old_top_mod_name = as_file_name_string(&orig)?;

    let old_top_mod_path = orig
        .as_ref()
        .parent()
        .ok_or_else(|| {
            format!("Failed to diff module file, no parent dir found for out dir {orig_root:?}")
        })?
        .join(format!("{old_top_mod_name}.rs"));
    match fs::read(&old_top_mod_path) {
        Ok(content) => {
            if content != new_mod.as_bytes() {
                diff += 1;
            }
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => diff += 1,
        Err(e) => {
            return Err(format!(
                "Failed to read old mod file at {old_top_mod_path:?} {e}"
            ));
        }
    };

    for _ in orig_files {
        diff += 1;
    }
    Ok(diff)
}

#[inline]
fn vec_remove(needle: &PathBuf, haystack: &mut Vec<PathBuf>) -> bool {
    for i in 0..haystack.len() {
        if needle == &haystack[i] {
            haystack.swap_remove(i);
            return true;
        }
    }
    false
}

fn collect_files(source: impl AsRef<Path> + Debug, root: &str) -> Result<Vec<PathBuf>, String> {
    let rd = fs::read_dir(&source);
    match rd {
        Ok(rd) => {
            let mut all_files = Vec::new();
            for entry in rd {
                let entry = entry.map_err(|e| {
                    format!("Failed to read entry when checking for file diff at {source:?} {e}")
                })?;
                let entry_path = entry.path();
                let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata for entry {entry_path:?} when checking for file diff at {source:?} {e}"))?;
                if metadata.is_file() {
                    let pb = path_from_starts_with(root, &entry_path)?;
                    all_files.push(pb);
                } else if metadata.is_dir() {
                    all_files.extend(collect_files(entry_path, root)?);
                } else {
                    return Err(format!("Found something that's neither a file or dir at {entry_path:?} while recursively collecting files at {source:?}"));
                }
            }
            Ok(all_files)
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!(
            "Got error reading dir {source:?} to check diff {e}"
        )),
    }
}

fn recurse_copy_clean(
    source: impl AsRef<Path> + Debug,
    dest: impl AsRef<Path> + Debug,
) -> Result<(), String> {
    if dest.as_ref().exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to clean out old dir {dest:?} {e}"))?;
        fs::create_dir(&dest)
            .map_err(|e| format!("Failed to create new proto dir {dest:?} {e}"))?;
    }

    let source_top = source.as_ref();
    let dest_top = dest.as_ref();
    if let Ok(metadata) = dest_top.metadata() {
        if !metadata.is_dir() {
            return Err(format!(
                "Destination {dest_top:?} exists but is not a directory"
            ));
        }
    } else {
        fs::create_dir_all(dest_top)
            .map_err(|e| format!("Failed to create generated output destination directory {e}"))?;
    }
    for entry in fs::read_dir(&source).map_err(|e| {
        format!("Failed to read source dir {source_top:?} to copy generated protos {e}")
    })? {
        let entry =
            entry.map_err(|e| format!("Failed to read entry to copy generated protos {e}"))?;
        recurse_copy_over(dest_top, entry.path())?;
    }

    Ok(())
}

fn recurse_copy_over(transfer_top: &Path, entry: impl AsRef<Path> + Debug) -> Result<(), String> {
    let path = entry.as_ref();
    let metadata = path.metadata().map_err(|e| {
        format!("Failed to get metadata for {path:?} to copy to generated protos from {e}")
    })?;
    let last_component = path
        .file_name()
        .ok_or_else(|| format!("Failed to find file name in path {path:?}"))?;
    let new_dir = transfer_top.join(last_component);
    if metadata.is_file() {
        fs::copy(path, &new_dir).map_err(|e| {
            format!("Failed to copy generated file from {path:?} to {new_dir:?} E: {e}")
        })?;
        Ok(())
    } else if metadata.is_dir() {
        fs::create_dir_all(&new_dir).map_err(|e| {
            format!("Failed to create dir to place generated proto at {new_dir:?} {e}")
        })?;
        for entry in fs::read_dir(path)
            .map_err(|e| format!("Failed to read dir while recursively copying {e}"))?
        {
            let entry =
                entry.map_err(|e| format!("Failed to read entry while recursively copying {e}"))?;
            recurse_copy_over(&new_dir, entry.path())?;
        }
        Ok(())
    } else {
        Err(format!(
            "Found path which is neither a dir nor a file when copying generated protos {path:?} {metadata:?}"
        ))
    }
}

#[inline]
fn path_from_starts_with(root: &str, path: impl AsRef<Path> + Debug) -> Result<PathBuf, String> {
    let mut components = path.as_ref().components();
    let mut found_root = false;
    for component in components.by_ref() {
        let out_str = component.as_os_str();
        let out_str = out_str
            .to_str()
            .ok_or_else(|| format!("Failed to convert generate file name '{out_str:?}' to utf8"))?;
        if out_str.starts_with(root) {
            found_root = true;
            break;
        }
    }
    if !found_root {
        return Err(format!(
            "Failed to trim path up to {root} for proto generated file at {path:?}"
        ));
    }
    let pb = components.collect::<PathBuf>();
    Ok(pb)
}

fn recurse_fmt(base: impl AsRef<Path>) -> Result<(), String> {
    let path = base.as_ref();
    for file in
        fs::read_dir(path).map_err(|e| format!("failed to read_dir for path {path:?} {e}"))?
    {
        let entry = file.map_err(|e| format!("Failed to read entry in paht {path:?} {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata for entry {entry:?} {e}"))?;
        let path = entry.path();
        if metadata.is_file() && has_ext(&path, "rs") {
            let out = std::process::Command::new("rustfmt")
                .arg(path)
                .arg("--edition")
                .arg("2021")
                .output()
                .map_err(|e| format!("Failed to format generated code {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "Failed to format, rustfmt returned error status {} with stderr {:?}",
                    out.status,
                    String::from_utf8(out.stderr)
                ));
            }
        } else if metadata.is_dir() {
            recurse_fmt(path)?;
        }
    }
    Ok(())
}

#[inline]
#[must_use]
pub fn has_ext(path: &Path, ext: &str) -> bool {
    path.file_name()
        .map_or(false, |p| p.eq_ignore_ascii_case(ext))
}
