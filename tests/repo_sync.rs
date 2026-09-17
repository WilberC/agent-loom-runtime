use agent_loom_runtime::repo_sync::{self, CAPABILITY};
use serde_json::json;
use std::process::Command;
use tempfile::TempDir;

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
fn accepts_push_operations() {
    for operation in ["push", "pull_push"] {
        assert!(
            repo_sync::parse(json!({
                "kind": CAPABILITY,
                "repository_url": "https://github.com/example/repo.git",
                "operation": operation,
                "reference": "main",
                "worktree": "repo"
            }))
            .is_ok()
        );
    }
}

#[tokio::test]
async fn pushes_existing_worktree_head_to_the_requested_reference() {
    let temp = TempDir::new().unwrap();
    let remote = temp.path().join("remote.git");
    let source = temp.path().join("source");
    let worktree = temp.path().join("worktree");
    git(temp.path(), ["init", "--bare", remote.to_str().unwrap()]);
    git(temp.path(), ["init", source.to_str().unwrap()]);
    git(&source, ["config", "user.email", "test@example.com"]);
    git(&source, ["config", "user.name", "Test User"]);
    std::fs::write(source.join("README.md"), "initial\n").unwrap();
    git(&source, ["add", "README.md"]);
    git(&source, ["commit", "-m", "initial"]);
    git(&source, ["branch", "-M", "main"]);
    git(
        &source,
        ["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&source, ["push", "origin", "main"]);
    git(
        temp.path(),
        [
            "clone",
            "--branch",
            "main",
            remote.to_str().unwrap(),
            worktree.to_str().unwrap(),
        ],
    );
    git(&worktree, ["config", "user.email", "test@example.com"]);
    git(&worktree, ["config", "user.name", "Test User"]);
    std::fs::write(worktree.join("README.md"), "updated\n").unwrap();
    git(&worktree, ["commit", "-am", "update"]);

    let instruction = repo_sync::SyncInstruction {
        kind: CAPABILITY.to_owned(),
        repository_url: remote.to_string_lossy().into_owned(),
        operation: repo_sync::Operation::Push,
        reference: "main".to_owned(),
        worktree: "worktree".into(),
        credential: None,
    };
    repo_sync::run(instruction, temp.path()).await.unwrap();

    let remote_head = git(&remote, ["rev-parse", "refs/heads/main"]);
    let local_head = git(&worktree, ["rev-parse", "HEAD"]);
    assert_eq!(remote_head, local_head);
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

fn git<const N: usize>(directory: &std::path::Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}
