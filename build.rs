use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=examples");
    println!("cargo:rerun-if-changed=fonts");
    println!("cargo:rerun-if-changed=ui/mobile.slint");

    if env::var("CARGO_FEATURE_MOBILE").is_ok() {
        slint_build::compile("ui/mobile.slint").map_err(|error| {
            io::Error::other(format!("failed to compile ui/mobile.slint: {error}"))
        })?;
    }

    let out_dir = required_env_path("OUT_DIR")?;
    let dest_path = out_dir.join("problem_files.rs");
    let font_dest_path = out_dir.join("bundled_font_files.rs");
    let manifest_dir = required_env_path("CARGO_MANIFEST_DIR")?;
    let examples_dir = manifest_dir.join("examples");
    let fonts_dir = manifest_dir.join("fonts");
    let problem_files = discover_problem_files(&manifest_dir, &examples_dir)?;
    let bundled_fonts = discover_bundled_fonts(&manifest_dir, &fonts_dir)?;

    let mut output = fs::File::create(&dest_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to create {}: {error}", dest_path.display()),
        )
    })?;
    write_problem_file_module(&mut output, &problem_files)?;
    let mut font_output = fs::File::create(&font_dest_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to create {}: {error}", font_dest_path.display()),
        )
    })?;
    write_bundled_font_module(&mut font_output, &bundled_fonts)?;

    Ok(())
}

fn required_env_path(name: &str) -> io::Result<PathBuf> {
    env::var_os(name).map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("required environment variable {name} is not set"),
        )
    })
}

fn discover_problem_files(
    manifest_dir: &Path,
    examples_dir: &Path,
) -> io::Result<Vec<(String, String)>> {
    let mut problem_files = Vec::new();
    if !examples_dir.is_dir() {
        return Ok(problem_files);
    }

    for entry in fs::read_dir(examples_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", examples_dir.display()),
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("ntq") {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());
        let file_stem = path_to_utf8(
            path.file_stem()
                .ok_or_else(|| invalid_path("problem file has no file stem", &path))?,
            &path,
        )?;
        let relative_path = path.strip_prefix(manifest_dir).map_err(|error| {
            invalid_path(&format!("path is outside manifest dir: {error}"), &path)
        })?;
        let relative_path = path_to_utf8(relative_path.as_os_str(), &path)?.replace('\\', "/");
        problem_files.push((file_stem.to_string(), relative_path));
    }

    problem_files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(problem_files)
}

fn path_to_utf8<'a>(value: &'a std::ffi::OsStr, path: &Path) -> io::Result<&'a str> {
    value.to_str().ok_or_else(|| {
        invalid_path(
            "problem paths must be valid UTF-8 to generate Rust source",
            path,
        )
    })
}

fn discover_bundled_fonts(
    manifest_dir: &Path,
    fonts_dir: &Path,
) -> io::Result<Vec<(String, String)>> {
    let mut fonts = Vec::new();
    if !fonts_dir.is_dir() {
        return Ok(fonts);
    }

    for entry in fs::read_dir(fonts_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read {}: {error}", fonts_dir.display()),
        )
    })? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        let ext = ext.to_ascii_lowercase();
        if ext != "ttf" && ext != "otf" {
            continue;
        }

        println!("cargo:rerun-if-changed={}", path.display());
        let stem = path_to_utf8(
            path.file_stem()
                .ok_or_else(|| invalid_path("font file has no file stem", &path))?,
            &path,
        )?;
        let relative_path = path.strip_prefix(manifest_dir).map_err(|error| {
            invalid_path(&format!("path is outside manifest dir: {error}"), &path)
        })?;
        let relative_path = path_to_utf8(relative_path.as_os_str(), &path)?.replace('\\', "/");
        fonts.push((stem.to_string(), relative_path));
    }

    fonts.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(fonts)
}

fn invalid_path(message: &str, path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{message}: {}", path.display()),
    )
}

fn write_problem_file_module(
    writer: &mut dyn Write,
    problem_files: &[(String, String)],
) -> io::Result<()> {
    writeln!(writer, "pub const PROBLEM_FILES_NAMES: &[&str] = &[")?;
    for (name, _) in problem_files {
        writeln!(writer, "    {name:?},")?;
    }
    writeln!(writer, "];\n")?;

    writeln!(
        writer,
        "pub fn get_problem_content(index: usize) -> &'static str {{"
    )?;
    writeln!(writer, "    match index {{")?;
    for (index, (_, path)) in problem_files.iter().enumerate() {
        writeln!(
            writer,
            "        {index} => include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/\", {path:?})),"
        )?;
    }
    writeln!(writer, "        _ => \"#title Error\\nFile not found.\",")?;
    writeln!(writer, "    }}")?;
    writeln!(writer, "}}")?;

    Ok(())
}

fn write_bundled_font_module(writer: &mut dyn Write, fonts: &[(String, String)]) -> io::Result<()> {
    writeln!(writer, "pub const BUNDLED_FONT_FILE_NAMES: &[&str] = &[")?;
    for (_, path) in fonts {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("font path has no file name: {path}"),
                )
            })?;
        writeln!(writer, "    {file_name:?},")?;
    }
    writeln!(writer, "];\n")?;

    writeln!(writer, "pub fn bundled_font_entries() -> Vec<FontEntry> {{")?;
    writeln!(writer, "    let mut entries = Vec::new();")?;
    for (index, (name, _)) in fonts.iter().enumerate() {
        writeln!(
            writer,
            "    entries.push(FontEntry {{ id: FontAssetId({index}), name: {name:?}.to_string(), source: FontSource::Bundled }});"
        )?;
    }
    writeln!(writer, "    entries")?;
    writeln!(writer, "}}\n")?;

    writeln!(
        writer,
        "pub fn bundled_font_file_name(id: FontAssetId) -> Option<&'static str> {{"
    )?;
    writeln!(writer, "    match id.0 {{")?;
    for (index, (_, path)) in fonts.iter().enumerate() {
        let file_name = Path::new(path)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("font path has no file name: {path}"),
                )
            })?;
        writeln!(writer, "        {index} => Some({file_name:?}),")?;
    }
    writeln!(writer, "        _ => None,")?;
    writeln!(writer, "    }}")?;
    writeln!(writer, "}}\n")?;

    writeln!(
        writer,
        "#[cfg(any(feature = \"wasi-tui\", feature = \"uefi\"))]"
    )?;
    writeln!(
        writer,
        "pub fn bundled_font_data(id: FontAssetId) -> Option<&'static [u8]> {{"
    )?;
    writeln!(writer, "    match id.0 {{")?;
    for (index, (_, path)) in fonts.iter().enumerate() {
        writeln!(
            writer,
            "        {index} => Some(include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/\", {path:?}))),"
        )?;
    }
    writeln!(writer, "        _ => None,")?;
    writeln!(writer, "    }}")?;
    writeln!(writer, "}}")?;

    Ok(())
}
