// The render-check script is strict TypeScript: it must pass `tsc --noEmit` with "strict": true.
// Ported from test/syntax.test.mjs, which parsed every shipped .mjs module; the Rust side is
// checked by the compiler, so what remains to check is the TypeScript.
mod common;
use common::*;
use std::process::Command;

/// The TypeScript compiler: verify/node_modules first (pnpm install --dir verify), then PATH.
fn tsc() -> String {
    let local = root_file("verify/node_modules/.bin/tsc");
    if std::path::Path::new(&local).exists() {
        return local;
    }
    "tsc".into()
}

fn type_check(project: &str) -> (bool, String) {
    let out = Command::new(tsc()).args(["--noEmit", "-p", project]).output().unwrap_or_else(|e| {
        panic!("could not run tsc ({e}). Install the type checker with: pnpm install --dir verify (or npm install --prefix verify)")
    });
    (out.status.success(), format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn the_verify_script_is_strict_and_type_checks() {
    let config: serde_json::Value = serde_json::from_str(&read(&root_file("verify/tsconfig.json"))).unwrap();
    assert_eq!(config["compilerOptions"]["strict"], true, "verify/tsconfig.json must set \"strict\": true");
    let (ok, output) = type_check(&root_file("verify/tsconfig.json"));
    assert!(ok, "tsc --noEmit failed:\n{output}");
}

#[test]
fn the_type_check_reports_a_file_that_does_not_type_check() {
    let dir = temp_dir();
    write(&dir.join("good.ts"), "export const x: number = 1;\n");
    write(&dir.join("broken.ts"), "export const x: number = \"one\";\n");
    write(
        &dir.join("tsconfig.json"),
        r#"{ "compilerOptions": { "strict": true, "noEmit": true, "target": "ES2023", "module": "ESNext", "types": [] }, "files": ["good.ts", "broken.ts"] }"#,
    );
    let (ok, output) = type_check(&dir.join("tsconfig.json"));
    assert!(!ok, "a type error went unreported");
    assert!(output.contains("broken.ts"), "{output}");
    assert!(!output.contains("good.ts"), "{output}");
}
