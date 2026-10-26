//! CARs rooted at an immutable head, never an archive of historical blocks.

use std::collections::BTreeSet;

use atproto_dasl::{BlockStorage, CarBlock, CarWriter, Cid};
use atproto_repo::RepoConfig;
use atproto_repo::mst::{Mst, MstNode};
use motet_rules::Did;

use crate::repo::Head;
use crate::store::{ReadBlocks, Reader, keys};

use super::{Error, internal};

async fn block(storage: &ReadBlocks, cid: Cid) -> Result<Vec<u8>, Error> {
    storage
        .get(&cid)
        .await
        .map_err(internal)?
        .ok_or_else(|| internal("a reachable repository block is missing"))
}

/// Record links are data, not edges of the repository's MST. In particular,
/// following a blob or strong reference would export another object graph.
async fn reachable(storage: &ReadBlocks, root: Cid) -> Result<BTreeSet<Cid>, Error> {
    let mut seen = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(cid) = pending.pop() {
        if !seen.insert(cid) {
            continue;
        }
        let node: MstNode = atproto_dasl::from_slice(&block(storage, cid).await?).map_err(internal)?;
        pending.extend(node.left);
        for entry in node.entries {
            seen.insert(entry.value);
            pending.extend(entry.tree);
        }
    }
    Ok(seen)
}

pub(super) async fn repo(reader: &Reader, did: &Did, head: &Head, since: Option<&str>) -> Result<Vec<u8>, Error> {
    let storage = reader.blocks(did);
    let baseline = if let Some(since) = since {
        let bytes = reader
            .get(&keys::repo_revision(did, since))
            .await
            .map_err(internal)?
            .ok_or_else(|| Error::InvalidRequest("unknown repository revision in since".into()))?;
        let previous: Head = serde_json::from_slice(&bytes).map_err(internal)?;
        reachable(&storage, previous.data).await?
    } else {
        BTreeSet::new()
    };
    let mut writer = CarWriter::new(Vec::new(), vec![head.cid]).await.map_err(internal)?;
    writer
        .write_block(&CarBlock::new(head.cid, block(&storage, head.cid).await?))
        .await
        .map_err(internal)?;
    let mut seen = BTreeSet::new();
    let mut pending = vec![head.data];
    while let Some(cid) = pending.pop() {
        // Equal node CIDs commit to equal subtrees, so the whole branch was
        // already present at `since` and need not be read again.
        if baseline.contains(&cid) || !seen.insert(cid) {
            continue;
        }
        let data = block(&storage, cid).await?;
        let node: MstNode = atproto_dasl::from_slice(&data).map_err(internal)?;
        writer.write_block(&CarBlock::new(cid, data)).await.map_err(internal)?;
        pending.extend(node.left);
        for entry in node.entries {
            if !baseline.contains(&entry.value) && seen.insert(entry.value) {
                let data = block(&storage, entry.value).await?;
                writer
                    .write_block(&CarBlock::new(entry.value, data))
                    .await
                    .map_err(internal)?;
            }
            pending.extend(entry.tree);
        }
    }
    writer.finish().await.map_err(internal)
}

/// Returns the signed head and a covering proof of presence or absence.
/// Present records include their block; missing reachable blocks are errors.
pub(super) async fn proof(reader: &Reader, did: &Did, head: &Head, path: &str) -> Result<Vec<u8>, Error> {
    let mst = Mst::from_root(head.data, reader.blocks(did), RepoConfig::default());
    let record = mst.get(path).await.map_err(internal)?;
    let proof = mst.covering_proof(path).await.map_err(internal)?;
    let mut nodes: Vec<_> = proof.into_iter().collect();
    nodes.sort_unstable_by_key(|(cid, _)| *cid);
    let mut writer = CarWriter::new(Vec::new(), vec![head.cid]).await.map_err(internal)?;
    writer
        .write_block(&CarBlock::new(head.cid, block(mst.storage(), head.cid).await?))
        .await
        .map_err(internal)?;
    for (cid, data) in nodes {
        writer.write_block(&CarBlock::new(cid, data)).await.map_err(internal)?;
    }
    if let Some(record) = record {
        writer
            .write_block(&CarBlock::new(record, block(mst.storage(), record).await?))
            .await
            .map_err(internal)?;
    }
    writer.finish().await.map_err(internal)
}

pub(super) async fn blocks(reader: &Reader, did: &Did, cids: Vec<Cid>) -> Result<Vec<u8>, Error> {
    let storage = reader.blocks(did);
    // The writer owns the header's roots; retain the requested CIDs for the
    // subsequent async reads without retaining a second copy of block data.
    let mut writer = CarWriter::new(Vec::new(), cids.clone()).await.map_err(internal)?;
    for cid in cids {
        let data = storage.get(&cid).await.map_err(internal)?.ok_or(Error::BlockNotFound)?;
        writer.write_block(&CarBlock::new(cid, data)).await.map_err(internal)?;
    }
    writer.finish().await.map_err(internal)
}
