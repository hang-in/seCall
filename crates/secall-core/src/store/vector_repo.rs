use crate::search::vector::VectorRow;

pub trait VectorRepo {
    fn init_vector_table(&self) -> anyhow::Result<()>;
    fn insert_vector(
        &self,
        embedding: &[f32],
        session_id: &str,
        turn_index: u32,
        chunk_seq: u32,
        model: &str,
    ) -> anyhow::Result<i64>;
    fn search_vectors(
        &self,
        query_embedding: &[f32],
        limit: usize,
        session_ids: Option<&[String]>,
    ) -> crate::error::Result<Vec<VectorRow>>;
    /// rowid로 turn_vectors의 (session_id, turn_index, chunk_seq) 조회.
    /// ANN 검색 결과를 DB 메타데이터와 연결할 때 사용.
    fn get_vector_meta(&self, rowid: i64) -> anyhow::Result<(String, u32, u32)>;
}
