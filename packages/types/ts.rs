use std::{
    ffi::OsStr,
    fs::{self, File},
    io::Write,
};

fn main() {
    // Generate index.ts
    let exports: Vec<_> = fs::read_dir("./packages/types/bindings")
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|p| {
            p.path()
                .file_stem()
                .and_then(OsStr::to_str)
                .map(str::to_owned)
        })
        .map(|f| format!("export * from \"./{}\"", f))
        .collect();

    let mut file = File::create("./packages/types/bindings/index.ts").unwrap();
    file.write_all(exports.join("\n").as_bytes()).unwrap();

    // Generate package.json with dynamic version from Cargo
    let version = env!("CARGO_PKG_VERSION");
    let package_json = create_package_json(version);
    let mut package_file = File::create("./packages/types/bindings/package.json").unwrap();
    package_file.write_all(package_json.as_bytes()).unwrap();
}

fn create_package_json(version: &str) -> String {
    format!(
        r#"{{
  "name": "@wavs/types",
  "version": "{version}",
  "description": "TypeScript type definitions for WAVS",
  "main": "index.js",
  "types": "index.ts",
  "files": [
    "*.ts"
  ],
  "author": "Layer Labs (Cayman)",
  "license": "GPL-3.0-or-later",
  "repository": {{
    "type": "git",
    "url": "git+https://github.com/Lay3rLabs/WAVS.git",
    "directory": "packages/types/bindings"
  }},
  "homepage": "https://github.com/Lay3rLabs/WAVS/tree/main/packages/types",
  "publishConfig": {{
    "access": "public"
  }},
  "devDependencies": {{
    "typescript": "^5.0.0"
  }}
}}
"#,
        version = version
    )
}
