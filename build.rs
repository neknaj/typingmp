use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("problem_files.rs");
    let mut f = fs::File::create(&dest_path).unwrap();

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let examples_dir = manifest_dir.join("examples");
    let mut problem_files = Vec::new();

    if examples_dir.is_dir() {
        for entry in fs::read_dir(examples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("ntq") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    // プロジェクトルートからの相対パスを保存
                    let relative_path = path.strip_prefix(&manifest_dir).unwrap().to_str().unwrap().replace('\\', "/");
                    problem_files.push((file_stem.to_string(), relative_path.to_string()));
                }
            }
        }
    }
    
    // ファイル名でソート
    problem_files.sort_by(|a, b| a.0.cmp(&b.0));

    // 問題ファイルの名前の静的配列を生成
    writeln!(f, "pub const PROBLEM_FILES_NAMES: &[&str] = &[").unwrap();
    for (name, _) in &problem_files {
        writeln!(f, "    \"{}\",", name).unwrap();
    }
    writeln!(f, "];\n").unwrap();

    // 問題ファイルの内容を動的に取得するための関数を生成
    writeln!(f, "pub fn get_problem_content(index: usize) -> &'static str {{").unwrap();
    writeln!(f, "    match index {{").unwrap();
    for (i, (_, path)) in problem_files.iter().enumerate() {
        // include_str! にはプロジェクトルートからの相対パスを渡す
        writeln!(f, "        {} => include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/{}\")),", i, path).unwrap();
    }
    writeln!(f, "        _ => \"#title Error\\nFile not found.\",").unwrap();
    writeln!(f, "    }}").unwrap();
    writeln!(f, "}}").unwrap();
}