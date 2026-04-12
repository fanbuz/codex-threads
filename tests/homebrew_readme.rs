use std::fs;

#[test]
fn readme_documents_homebrew_installation() {
    let readme =
        fs::read_to_string("README.md").expect("README.md should be readable from repo root");

    assert!(readme.contains("brew tap fanbuz/tap"));
    assert!(readme.contains("brew install fanbuz/tap/codex-threads"));
    assert!(readme.contains("brew upgrade codex-threads"));
    assert!(readme.contains("支持平台直接安装预编译二进制"));
    assert!(readme.contains("否则回退源码构建"));
    assert!(readme.contains("macOS x64"));
}
