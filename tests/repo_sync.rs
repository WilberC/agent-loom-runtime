use agent_loom_runtime::repo_sync::{self, CAPABILITY};
use serde_json::json;

#[test]
fn accepts_pull_instruction_with_relative_worktree() {
    let instruction = repo_sync::parse(json!({
        "kind": CAPABILITY,
        "repository_url": "https://github.com/WilberC/dotfiles-skills.git",
        "operation": "pull",
        "reference": "main",
        "worktree": "dotfiles-skills",
        "credential": {"username": "x-access-token", "token": "secret"}
    }))
    .unwrap();
    assert_eq!(
        instruction.repository_url,
        "https://github.com/WilberC/dotfiles-skills.git"
    );
}

#[test]
fn rejects_unsafe_repository_sync_inputs() {
    for (url, worktree, reference) in [
        ("http://github.com/example/repo.git", "repo", "main"),
        ("https://github.com/example/repo.git", "../repo", "main"),
        ("https://github.com/example/repo.git", "repo", "main branch"),
    ] {
        assert!(
            repo_sync::parse(json!({
                "kind": CAPABILITY,
                "repository_url": url,
                "operation": "pull",
                "reference": reference,
                "worktree": worktree
            }))
            .is_err()
        );
    }
}
