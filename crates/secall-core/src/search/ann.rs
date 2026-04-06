use anyhow::Result;
use std::path::{Path, PathBuf};
use usearch::{new_index, Index, IndexOptions, MetricKind, ScalarKind};

pub struct AnnIndex {
    index: Index,
    path: PathBuf,
    dimensions: usize,
}

impl AnnIndex {
    /// 기존 인덱스 파일이 있으면 로드, 없으면 새로 생성.
    pub fn open_or_create(path: &Path, dimensions: usize) -> Result<Self> {
        let options = IndexOptions {
            dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: 0,
            expansion_add: 0,
            expansion_search: 0,
            multi: false,
        };
        let index = new_index(&options).map_err(|e| anyhow::anyhow!("{e}"))?;

        if path.exists() {
            let path_str = path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF-8 ANN index path: {:?}", path))?;
            index.load(path_str).map_err(|e| anyhow::anyhow!("{e}"))?;
            tracing::info!(
                path = %path.display(),
                vectors = index.size(),
                "ANN index loaded"
            );
        } else {
            index.reserve(10_000).map_err(|e| anyhow::anyhow!("{e}"))?;
            tracing::info!(path = %path.display(), "ANN index created (empty)");
        }

        Ok(Self {
            index,
            path: path.to_path_buf(),
            dimensions,
        })
    }

    /// 벡터 추가. key는 turn_vectors 테이블의 rowid.
    pub fn add(&self, key: u64, vector: &[f32]) -> Result<()> {
        self.index
            .add(key, vector)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// ANN 검색. 상위 limit개의 (key, distance) 반환.
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<(u64, f32)>> {
        let results = self
            .index
            .search(query, limit)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(results.keys.into_iter().zip(results.distances).collect())
    }

    /// 인덱스를 파일에 저장.
    pub fn save(&self) -> Result<()> {
        let path_str = self
            .path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 ANN index path: {:?}", self.path))?;
        self.index
            .save(path_str)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        tracing::info!(
            path = %self.path.display(),
            vectors = self.index.size(),
            "ANN index saved"
        );
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.index.size()
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_ann_create_add_search() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.usearch");

        let ann = AnnIndex::open_or_create(&path, 3).unwrap();
        assert_eq!(ann.size(), 0);
        assert_eq!(ann.dimensions(), 3);

        ann.add(1, &[1.0_f32, 0.0, 0.0]).unwrap();
        ann.add(2, &[0.0_f32, 1.0, 0.0]).unwrap();
        assert_eq!(ann.size(), 2);

        let results = ann.search(&[1.0_f32, 0.1, 0.0], 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1); // closer to [1,0,0]
    }

    #[test]
    fn test_ann_save_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.usearch");

        {
            let ann = AnnIndex::open_or_create(&path, 3).unwrap();
            ann.add(42, &[1.0_f32, 0.0, 0.0]).unwrap();
            ann.save().unwrap();
        }

        // Reload from file
        let ann2 = AnnIndex::open_or_create(&path, 3).unwrap();
        assert_eq!(ann2.size(), 1);
        let results = ann2.search(&[1.0_f32, 0.0, 0.0], 1).unwrap();
        assert_eq!(results[0].0, 42);
    }
}
