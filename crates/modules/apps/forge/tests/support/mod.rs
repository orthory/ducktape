use std::sync::atomic::{AtomicU64, Ordering};

use git2::{Buf, Repository, RepositoryInitOptions, Signature, Time};

static NEXT_REPO: AtomicU64 = AtomicU64::new(0);

pub struct PackedCommit {
    pub head: Vec<u8>,
    pub pack: Vec<u8>,
}

/// Build ordinary Git commits outside Forge and return each head's full object
/// closure. Production obtains the same shape from stock `git push`; tests use
/// libgit2 directly so the consensus module never needs a commit-building API.
pub fn history(tag: &str, changes: &[(u64, &str, &str, &str)]) -> Vec<PackedCommit> {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "ducktape-forge-fixture-{tag}-{}-{}",
        std::process::id(),
        NEXT_REPO.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);

    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main").external_template(false);
    let repo = Repository::init_opts(&dir, &opts).unwrap();
    let mut head = None;
    let mut out = Vec::with_capacity(changes.len());

    for &(timestamp, path, content, message) in changes {
        let parent = head.map(|oid| repo.find_commit(oid).unwrap());
        let base_tree = parent.as_ref().map(|commit| commit.tree().unwrap());
        let blob = repo.blob(content.as_bytes()).unwrap();
        let tree_oid = {
            let mut builder = repo.treebuilder(base_tree.as_ref()).unwrap();
            builder.insert(path, blob, 0o100644).unwrap();
            builder.write().unwrap()
        };
        let tree = repo.find_tree(tree_oid).unwrap();
        let time = Time::new(i64::try_from(timestamp).unwrap(), 0);
        let signature = Signature::new("ducktape", "ducktape@localhost", &time).unwrap();
        let parents = parent.iter().collect::<Vec<_>>();
        let oid = repo
            .commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .unwrap();
        head = Some(oid);

        let mut packer = repo.packbuilder().unwrap();
        packer.set_threads(1);
        let mut walk = repo.revwalk().unwrap();
        walk.push(oid).unwrap();
        for object in walk {
            packer.insert_commit(object.unwrap()).unwrap();
        }
        let mut pack = Buf::new();
        packer.write_buf(&mut pack).unwrap();
        out.push(PackedCommit {
            head: oid.as_bytes().to_vec(),
            pack: pack.to_vec(),
        });
    }

    drop(repo);
    let _ = std::fs::remove_dir_all(dir);
    out
}
