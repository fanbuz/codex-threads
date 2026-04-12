use std::fs;
use std::path::Path;

#[test]
fn repository_includes_open_source_project_scaffolding() {
    for relative_path in [
        ".github/ISSUE_TEMPLATE/bug_report.md",
        ".github/ISSUE_TEMPLATE/feature_request.md",
        ".github/PULL_REQUEST_TEMPLATE.md",
        ".github/workflows/build.yml",
        ".github/workflows/release.yml",
        "CODE_OF_CONDUCT.md",
        "CONTRIBUTING.md",
        "SECURITY.md",
        "LICENSE",
        "Makefile",
    ] {
        assert!(
            Path::new(relative_path).exists(),
            "missing repository scaffolding file: {relative_path}"
        );
    }
}

#[test]
fn workflows_and_readme_are_aligned_with_codex_threads() {
    let readme = fs::read_to_string("README.md").expect("README.md should exist");
    assert!(readme.contains("[![License: MIT]"));
    assert!(readme.contains("## Contributing"));
    assert!(readme.contains("codex-threads --json sync"));

    let build =
        fs::read_to_string(".github/workflows/build.yml").expect("build workflow should exist");
    assert!(build.contains("BIN_NAME: codex-threads"));
    assert!(build.contains("cargo test --locked"));

    let release =
        fs::read_to_string(".github/workflows/release.yml").expect("release workflow should exist");
    assert!(release.contains("BIN_NAME: codex-threads"));
    assert!(release.contains("gh release create"));
    assert!(release.contains("x86_64-apple-darwin"));
    assert!(release.contains("codex-threads-macos-x64.tar.gz"));
    assert!(!release.contains("x86_64-pc-windows-msvc"));
    assert!(!release.contains("codex-threads-windows-x64.zip"));
    assert!(release.contains("notify-homebrew-tap"));
    assert!(release.contains("repository_dispatch"));
    assert!(release.contains("HOMEBREW_TAP_TOKEN"));
    assert!(release.contains("Check tap dispatch token"));
    assert!(release.contains("steps.token.outputs.available == 'true'"));
    assert!(release.contains("\"source_repository\": \"${GITHUB_REPOSITORY}\""));
    assert!(release.contains("\"tag\": \"${TAG_NAME}\""));
    assert!(release.contains("\"formula_name\": \"codex-threads\""));
    assert!(!release.contains("if: ${{ secrets.HOMEBREW_TAP_TOKEN != '' }}"));
    assert!(!release.contains("render_homebrew_formula.py"));
    assert!(!release.contains("Checkout Homebrew tap"));
}
