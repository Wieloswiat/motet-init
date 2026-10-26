//! Commit-event CAR selection, ported from atproto-pds `src/repo/commit_car.rs`
//! at revision 09a868cdde938c789326f1db16ffc30bbf38c15e (MIT).
//! Local changes are recorded in `crates/tenor/UPSTREAM.md`.

use std::collections::{BTreeMap, BTreeSet};

use atproto_dasl::{CarBlock, CarError, CarWriter, Cid};
use atproto_repo::mst::MstNode;

#[derive(Clone, Copy)]
enum BlockKind {
    Commit,
    Node,
    Record,
}

/// Edge kinds, not record shape, determine which links belong to this repo.
fn children(data: &[u8], kind: BlockKind) -> Result<Vec<(Cid, BlockKind)>, CarError> {
    let bad_block = |error: atproto_dasl::DecodeError| CarError::InvalidBlock {
        reason: error.to_string(),
    };
    match kind {
        BlockKind::Commit => {
            let commit = atproto_dasl::from_slice::<atproto_repo::Commit>(data).map_err(bad_block)?;
            Ok(vec![(commit.data, BlockKind::Node)])
        }
        BlockKind::Node => {
            let node = atproto_dasl::from_slice::<MstNode>(data).map_err(bad_block)?;
            let mut out = Vec::with_capacity(node.entries.len() * 2 + 1);
            if let Some(left) = node.left {
                out.push((left, BlockKind::Node));
            }
            for entry in node.entries {
                out.push((entry.value, BlockKind::Record));
                if let Some(right) = entry.tree {
                    out.push((right, BlockKind::Node));
                }
            }
            Ok(out)
        }
        BlockKind::Record => Ok(Vec::new()),
    }
}

/// The signed commit and reachable new blocks, unioned with covering proofs.
/// Intermediate MST nodes from earlier writes in the batch are not published.
pub(super) async fn build_commit_car(
    root: Cid,
    commit_block: &[u8],
    written: &BTreeMap<Cid, Vec<u8>>,
    proof: &BTreeMap<Cid, Vec<u8>>,
) -> Result<Vec<u8>, CarError> {
    let mut buffer = Vec::new();
    let mut writer = CarWriter::new(&mut buffer, vec![root]).await?;
    let mut visited = BTreeSet::new();
    let mut stack = vec![(root, BlockKind::Commit)];
    while let Some((cid, kind)) = stack.pop() {
        if !visited.insert(cid) {
            continue;
        }
        let data = if cid == root {
            commit_block
        } else if let Some(data) = written.get(&cid).or_else(|| proof.get(&cid)) {
            data
        } else {
            // The consumer already holds this unchanged subtree.
            continue;
        };
        stack.extend(children(data, kind)?);
        // CarWriter accepts an owned CarBlock, while the caller still needs
        // these bytes to persist the same commit atomically.
        writer
            .write_block(&CarBlock {
                cid,
                data: data.to_vec(),
            })
            .await?;
    }
    writer.finish().await?;
    Ok(buffer)
}

/// Sync re-anchors on a signed commit; clients fetch its tree through getRepo.
pub(super) async fn build_sync_car(root: Cid, commit_block: &[u8]) -> Result<Vec<u8>, CarError> {
    let mut buffer = Vec::with_capacity(commit_block.len() + 128);
    let mut writer = CarWriter::new(&mut buffer, vec![root]).await?;
    writer
        .write_block(&CarBlock {
            cid: root,
            data: commit_block.to_vec(),
        })
        .await?;
    writer.finish().await?;
    Ok(buffer)
}
