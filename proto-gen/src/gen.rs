//! A library that generates Rust code using tonic-build and places that code in a supplied directory
#![warn(clippy::pedantic)]
#![allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    clippy::unnecessary_debug_formatting
)]

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt::{Debug, Write};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use tonic_prost_build::Builder;

/// Generate protos for the provided proto workspace
/// # Errors
/// Miscellaneous errors accessing the filesystem (such as permissions),
/// and errors coming from `protoc`
pub fn run_generation(
    proto_ws: &ProtoWorkspace,
    opts: Builder,
    config: tonic_prost_build::Config,
    gen_opts: &GenOptions,
) -> Result<(), String> {
    let mut top_mod_content = generate_to_tmp(proto_ws, opts, config, gen_opts).map_err(|e| {
        format!("Failed to generate protos into temp dir for proto workspace {proto_ws:#?} \n{e}")
    })?;
    let old = &proto_ws.output_dir;
    let new = &proto_ws.tmp_dir;
    if let Some(edition) = gen_opts.format.as_deref() {
        recurse_fmt(new, edition)?;
        top_mod_content = fmt(&top_mod_content, edition)?;
    }
    let diff = run_diff(old, new, &top_mod_content)?;
    if diff > 0 {
        println!("Found diff in {diff} protos at {:?}", proto_ws.output_dir);
        if gen_opts.commit {
            println!("Writing {diff} protos to {:?}", proto_ws.output_dir);
            recurse_copy_clean(new, old)?;
            let out_top_name = as_file_name_string(old)?;
            let out_parent = old.parent().ok_or_else(|| {
                format!("Failed to find parent for output dir {old:?} to place mod file")
            })?;
            let mod_file = out_parent.join(format!("{out_top_name}.rs"));
            fs::write(&mod_file, top_mod_content.as_bytes())
                .map_err(|e| format!("Failed to write parent module file to {mod_file:?} \n{e}"))?;
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

#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct GenOptions {
    pub commit: bool,
    pub format: Option<String>,
    pub prepend_header: Option<String>,
    pub toplevel_attribute: Option<String>,
}

fn generate_to_tmp(
    ws: &ProtoWorkspace,
    opts: Builder,
    config: tonic_prost_build::Config,
    gen_opts: &GenOptions,
) -> Result<String, String> {
    let old_out = std::env::var("OUT_DIR");
    std::env::set_var("OUT_DIR", &ws.tmp_dir);
    // Would by nice if we could just get a byte buffer instead of magic env write
    opts.compile_with_config(config, &ws.proto_files, &ws.proto_dirs)
        .map_err(|e| format!("Failed to compile protos from {:#?} \n{e}", ws.proto_dirs))?;
    // Restore the env, cause why not
    if let Ok(old) = old_out {
        std::env::set_var("OUT_DIR", old);
    } else {
        std::env::remove_var("OUT_DIR");
    }

    clean_up_file_structure(&ws.tmp_dir, gen_opts)
}

fn clean_up_file_structure(out_dir: &Path, gen_opts: &GenOptions) -> Result<String, String> {
    let rd = fs::read_dir(out_dir)
        .map_err(|e| format!("Failed read output dir {out_dir:?} when cleaning up files \n{e}"))?;
    let mut out_modules = Module {
        name: "dummy".to_string(),
        location: out_dir.to_path_buf(),
        children: HashMap::new(),
        file: None,
    };
    for entry in rd {
        let entry = entry.map_err(|e| {
            format!(
                "Failed to read DirEntry when cleaning up output dir {:?} \n{e}",
                &out_dir
            )
        })?;
        let file_path = entry.path();
        let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata for entity {file_path:?} in output dir {out_dir:?} when cleaning up files \n{e}"))?;
        if metadata.is_file() {
            // Tonic build 0.7 generates a bunch of empty files for some reason, fixed in 0.8
            let content = fs::read(&file_path).map_err(|e| {
                format!("Failed to read generated file at path {file_path:?} \n{e}")
            })?;
            if content.is_empty() {
                fs::remove_file(&file_path).map_err(|e| {
                    format!("Failed to delete empty file {file_path:?} from temp directory \n{e}")
                })?;
            } else {
                out_modules.push_file(out_dir, &file_path)?;
            }
        }
    }
    let mut sortable_children = out_modules
        .children
        .into_values()
        .collect::<Vec<Rc<RefCell<Module>>>>();
    // Linting, guh
    let mut top_level_mod = String::new();
    prepend_header(gen_opts.prepend_header.as_ref(), &mut top_level_mod);
    top_level_mod.push_str("#![allow(clippy::doc_markdown, clippy::use_self)]\n");

    if let Some(toplevel_attribute) = &gen_opts.toplevel_attribute {
        top_level_mod.push_str(toplevel_attribute);
        top_level_mod.push('\n');
    }

    sortable_children.sort_by(|a, b| a.borrow().get_name().cmp(b.borrow().get_name()));
    for module in sortable_children {
        module.borrow_mut().dump_to_disk(gen_opts)?;
        let _ = top_level_mod.write_fmt(format_args!("pub mod {};\n", module.borrow().get_name()));
    }
    Ok(top_level_mod)
}

#[derive(Debug)]
struct Module {
    name: String,
    location: PathBuf,
    children: HashMap<String, Rc<RefCell<Module>>>,
    file: Option<PathBuf>,
}

impl Module {
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
            let new_parent = parent.join(cur);
            if let Some(child) = self.children.get(cur) {
                child.borrow_mut().push_recurse(&new_parent, path, rest)?;
            } else {
                let md = Rc::new(RefCell::new(Module {
                    name: cur.to_string(),
                    location: parent.to_path_buf(),
                    children: HashMap::new(),
                    file: None,
                }));
                self.children.insert(cur.to_string(), md.clone());
                md.borrow_mut().push_recurse(&new_parent, path, rest)?;
            }
        } else if let Some(old) = self.children.get(raw_name) {
            assert!(old.borrow().file.is_none(), "Logic error");
            old.borrow_mut().file = Some(path.as_ref().to_path_buf());
        } else {
            self.children.insert(
                raw_name.to_string(),
                Rc::new(RefCell::new(Module {
                    name: raw_name.to_string(),
                    location: parent.to_path_buf(),

                    children: HashMap::default(),
                    file: Some(path.as_ref().to_path_buf()),
                })),
            );
        }
        Ok(())
    }

    fn dump_to_disk(&self, gen_opts: &GenOptions) -> Result<(), String> {
        let module_expose_output = if self.children.is_empty() {
            None
        } else {
            let dir = self.location.join(&self.name);
            fs::create_dir_all(&dir)
                .map_err(|e| format!("Failed to create module directory for {dir:?} \n{e}"))?;
            let mut sortable_children = self
                .children
                .values()
                .collect::<Vec<&Rc<RefCell<Module>>>>();
            sortable_children.sort_by(|a, b| {
                let a_borrow = a.borrow();
                let b_borrow = b.borrow();
                a_borrow.get_name().cmp(b_borrow.get_name())
            });
            let mut output = String::new();
            prepend_header(gen_opts.prepend_header.as_ref(), &mut output);
            for sorted_child in sortable_children {
                let _ = output.write_fmt(format_args!(
                    "pub mod {};\n",
                    sorted_child.borrow().get_name()
                ));
                sorted_child.borrow().dump_to_disk(gen_opts)?;
            }
            Some(output)
        };
        if let Some(file) = self.file.as_ref() {
            let file_location = self
                .location
                .join(format!("{}.rs", self.proper_file_name()));
            // It's the same filename we don't need to move it but we need to edit it if it has
            // child modules.
            let is_same_file = &file_location == file;
            if let Some(mut module_header) = module_expose_output {
                let file_content = fs::read_to_string(file)
                    .map_err(|e| format!("Failed to read created file {file:?} \n{e}"))?;
                module_header.push('\n');
                module_header.push_str(&file_content);
                let mut clean = hide_doctests(&module_header);

                prepend_header(gen_opts.prepend_header.as_ref(), &mut clean);

                fs::write(&file_location, clean.as_bytes()).map_err(|e| {
                    format!("Failed to write file contents to {file_location:?} \n{e}")
                })?;
                // Don't remove if same file
                if !is_same_file {
                    fs::remove_file(file).map_err(|e| {
                        format!("Failed to remove original file from {file:?} \n{e}")
                    })?;
                }
                // Don't try to copy into self, will get empty file
            } else {
                let file_content = fs::read_to_string(file)
                    .map_err(|e| format!("Failed to read created file {file:?} \n{e}"))?;
                fs::remove_file(file)
                    .map_err(|e| format!("Failed to remove original file from {file:?} \n{e}"))?;

                let mut clean_content = hide_doctests(&file_content);

                prepend_header(gen_opts.prepend_header.as_ref(), &mut clean_content);

                fs::write(&file_location, clean_content.as_bytes()).map_err(|e| {
                    format!("Failed to write file contents to {file_location:?} \n{e}")
                })?;
            }
        } else if let Some(module_header) = module_expose_output {
            let mod_file_location = self.location.join(format!("{}.rs", self.name));
            fs::write(&mod_file_location, module_header.as_bytes()).map_err(|e| {
                format!("Failed to write module file at {mod_file_location:?} \n{e}")
            })?;
        } else {
            panic!("Bad code");
        }
        Ok(())
    }

    #[inline]
    fn get_name(&self) -> &str {
        self.name.as_str()
    }

    #[inline]
    fn proper_file_name(&self) -> &str {
        if self.name.starts_with("r#") {
            &self.name[2..]
        } else {
            self.name.as_str()
        }
    }
}

fn prepend_header(maybe_prepend_header: Option<&String>, clean_content: &mut String) {
    if let Some(prepend_header) = maybe_prepend_header {
        clean_content.insert_str(0, prepend_header);
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
        if orig_files.remove(file) {
            let orig_path = orig.as_ref().join(file);
            let new_path = new.as_ref().join(file);
            let a = fs::read(&orig_path)
                .map_err(|e| format!("Failed to read file at {orig_path:?} \n{e}"))?;
            let b = fs::read(&new_path)
                .map_err(|e| format!("Failed to read file at {new_path:?} \n{e}"))?;
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
                "Failed to read old mod file at {old_top_mod_path:?} \n{e}"
            ));
        }
    }

    for _ in orig_files {
        diff += 1;
    }
    Ok(diff)
}

fn collect_files(source: impl AsRef<Path> + Debug, root: &str) -> Result<HashSet<PathBuf>, String> {
    let rd = fs::read_dir(&source);
    match rd {
        Ok(rd) => {
            let mut all_files = HashSet::new();
            for entry in rd {
                let entry = entry.map_err(|e| {
                    format!("Failed to read entry when checking for file diff at {source:?} \n{e}")
                })?;
                let entry_path = entry.path();
                let metadata = entry.metadata().map_err(|e| format!("Failed to get metadata for entry {entry_path:?} when checking for file diff at {source:?} \n{e}"))?;
                if metadata.is_file() {
                    let pb = path_from_starts_with(root, &entry_path)?;
                    all_files.insert(pb);
                } else if metadata.is_dir() {
                    all_files.extend(collect_files(entry_path, root)?);
                } else {
                    return Err(format!("Found something that's neither a file or dir at {entry_path:?} while recursively collecting files at {source:?}"));
                }
            }
            Ok(all_files)
        }
        Err(ref e) if e.kind() == ErrorKind::NotFound => Ok(HashSet::new()),
        Err(e) => Err(format!(
            "Got error reading dir {source:?} to check diff \n{e}"
        )),
    }
}

fn recurse_copy_clean(
    source: impl AsRef<Path> + Debug,
    dest: impl AsRef<Path> + Debug,
) -> Result<(), String> {
    if dest.as_ref().exists() {
        fs::remove_dir_all(&dest)
            .map_err(|e| format!("Failed to clean out old dir {dest:?} \n{e}"))?;
        fs::create_dir(&dest)
            .map_err(|e| format!("Failed to create new proto dir {dest:?} \n{e}"))?;
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
        fs::create_dir_all(dest_top).map_err(|e| {
            format!("Failed to create generated output destination directory \n{e}")
        })?;
    }
    for entry in fs::read_dir(&source).map_err(|e| {
        format!("Failed to read source dir {source_top:?} to copy generated protos \n{e}")
    })? {
        let entry =
            entry.map_err(|e| format!("Failed to read entry to copy generated protos \n{e}"))?;
        recurse_copy_over(dest_top, entry.path())?;
    }

    Ok(())
}

fn recurse_copy_over(dest_top: &Path, entry: impl AsRef<Path> + Debug) -> Result<(), String> {
    let path = entry.as_ref();
    let metadata = path.metadata().map_err(|e| {
        format!("Failed to get metadata for {path:?} to copy to generated protos from \n{e}")
    })?;
    let last_component = path
        .file_name()
        .ok_or_else(|| format!("Failed to find file name in path {path:?}"))?;
    let new_dir = dest_top.join(last_component);
    if metadata.is_file() {
        fs::copy(path, &new_dir).map_err(|e| {
            format!("Failed to copy generated file from {path:?} to {new_dir:?} \n{e}")
        })?;
        Ok(())
    } else if metadata.is_dir() {
        fs::create_dir_all(&new_dir).map_err(|e| {
            format!("Failed to create dir to place generated proto at {new_dir:?} \n{e}")
        })?;
        for entry in fs::read_dir(path)
            .map_err(|e| format!("Failed to read dir while recursively copying \n{e}"))?
        {
            let entry = entry
                .map_err(|e| format!("Failed to read entry while recursively copying \n{e}"))?;
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
    let mut components = path.as_ref().components().rev();
    let mut found_root = false;
    let mut backwards_components = vec![];
    for component in components.by_ref() {
        let out_str = component.as_os_str();
        let out_str = out_str
            .to_str()
            .ok_or_else(|| format!("Failed to convert generate file name '{out_str:?}' to utf8"))?;
        if out_str.starts_with(root) {
            found_root = true;
            break;
        }
        backwards_components.push(component);
    }
    if !found_root {
        return Err(format!(
        "Failed to trim path up to {root} for proto generated file at: {path:?}. Could not find {root}. "
    ));
    }
    backwards_components.reverse();
    let pb = backwards_components.into_iter().collect::<PathBuf>();
    Ok(pb)
}

fn recurse_fmt(base: impl AsRef<Path>, edition: &str) -> Result<(), String> {
    let path = base.as_ref();
    for file in
        fs::read_dir(path).map_err(|e| format!("failed to read_dir for path {path:?} \n{e}"))?
    {
        let entry = file.map_err(|e| format!("Failed to read entry in path {path:?} \n{e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("Failed to read metadata for entry {entry:?} \n{e}"))?;
        let path = entry.path();
        if metadata.is_file() && has_ext(&path, "rs") {
            let out = std::process::Command::new("rustfmt")
                .arg(&path)
                .arg("--edition")
                .arg(edition)
                .output()
                .map_err(|e| format!("Failed to format generated code \n{e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "Failed to format, rustfmt returned error status {} with stderr {:?}",
                    out.status,
                    String::from_utf8(out.stderr)
                ));
            }
        } else if metadata.is_dir() {
            recurse_fmt(path, edition)?;
        }
    }
    Ok(())
}

fn fmt(code: &str, edition: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = std::process::Command::new("rustfmt")
        .arg("--edition")
        .arg(edition)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to format, failed to launch rustfmt\n{e}"))?;

    let child_stdin = child.stdin.as_mut().unwrap();
    child_stdin
        .write_all(code.as_bytes())
        .map_err(|e| format!("Failed to format, failed to write data to rustfmt \n{e}"))?;
    // drop(child_stdin);

    let formatted_code = String::from_utf8(
        child
            .wait_with_output()
            .map_err(|e| format!("Failed to format, rustfmt failed to run \n{e}"))?
            .stdout,
    )
    .map_err(|e| format!("Failed to read formtted generated code \n{e}"))?;
    Ok(formatted_code)
}

/// Rustdoc assumes all comments with 4 or more spaces or three backticks are things it absolutely
/// should try to compile and run, which seems like an insane assumption, we try our best
/// to strip those symbols here.
fn hide_doctests(content: &str) -> String {
    let mut in_multiline_codeblock = false;
    let mut in_potentially_hostile_code = false;
    let mut new_content = String::with_capacity(content.len());
    for line in content.lines() {
        if line.ends_with("```") {
            if in_multiline_codeblock {
                in_multiline_codeblock = false;
            } else {
                in_multiline_codeblock = true;
                let _ = new_content.write_fmt(format_args!("{line}ignore\n"));
                continue;
            }
        }

        if !in_multiline_codeblock {
            if let Some((_com, rest)) = line.split_once("///") {
                if rest.len() >= 4 && rest.chars().take(4).all(char::is_whitespace) {
                    // If 4 or more spaces after comment Rustdoc will think its code it should compile
                    if !in_potentially_hostile_code {
                        // If first time, insert ```ignore
                        in_potentially_hostile_code = true;
                        new_content.push_str("///```ignore\n");
                    }
                } else if in_potentially_hostile_code {
                    // If not 4 whitespaces anymore, insert ignore end token
                    new_content.push_str("///```\n");
                    in_potentially_hostile_code = false;
                }
                // If no longer in comments, comment ended on 4+ whitespaces, insert another
            } else if in_potentially_hostile_code {
                new_content.push_str("///```\n");
                in_potentially_hostile_code = false;
            }
        }

        let _ = new_content.write_fmt(format_args!("{line}\n"));
    }
    new_content
}

#[inline]
#[must_use]
pub fn has_ext(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|p| p.eq_ignore_ascii_case(ext))
}

#[cfg(test)]
mod tests {
    use crate::gen::{path_from_starts_with, run_diff};
    use std::path::Path;

    #[test]
    fn can_find_path_from_some_root_exists() {
        let this_file = Path::new("src/gen.rs");
        let abs = this_file.canonicalize().unwrap();
        let root = "proto-gen";
        let found = path_from_starts_with(root, abs).unwrap();
        assert_eq!(this_file.to_path_buf(), found);
    }

    #[test]
    fn can_find_path_from_some_root_missing() {
        let this_file = Path::new("src/gen.rs");
        let abs = this_file.canonicalize().unwrap();
        let root = "not-found-af38cd-9fxzz7p-- ";
        assert!(path_from_starts_with(root, abs).is_err());
    }

    #[test]
    fn can_diff_both_empty() {
        let empty_temp1 = tempfile::tempdir().unwrap();
        let empty_temp2 = tempfile::tempdir().unwrap();
        let diff = run_diff(empty_temp1.path(), empty_temp2.path(), "my-mod").unwrap();
        // One diff, would write a module file
        assert_eq!(1, diff);
    }

    #[test]
    fn can_diff_identical() {
        let proto_mod = "proto_types";
        let my_mod = "my_mod";
        let expect_top_content = format!("pub mod {my_mod};\n");
        let orig = tempfile::tempdir().unwrap();
        let orig_mod_dir = orig.path().join(proto_mod);
        std::fs::create_dir(&orig_mod_dir).unwrap();
        std::fs::write(orig_mod_dir.join("my_mod.rs"), "!// Content").unwrap();
        std::fs::write(
            orig.path().join(format!("{proto_mod}.rs")),
            &expect_top_content,
        )
        .unwrap();
        let new = tempfile::tempdir().unwrap();
        let new_mod_dir = new.path().join(proto_mod);
        std::fs::create_dir(&new_mod_dir).unwrap();
        std::fs::write(
            new.path().join(format!("{proto_mod}.rs")),
            &expect_top_content,
        )
        .unwrap();
        std::fs::write(new_mod_dir.join("my_mod.rs"), "!// Content").unwrap();
        let diff = run_diff(&orig_mod_dir, &new_mod_dir, &expect_top_content).unwrap();
        assert_eq!(0, diff);
    }
}
