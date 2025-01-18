pub struct StreamTracker {
    pub last_stream_pos: u64,
    pub last_chunk_len: usize,
    pub buffer: Vec<u8>,
    pub chunk: Vec<u8>,
}
impl StreamTracker {
    pub fn new(chunk_size: usize) -> StreamTracker {
        StreamTracker {
            last_stream_pos: 0,
            last_chunk_len: 0,
            buffer: Vec::with_capacity(chunk_size * 2), // Extra space for overflow,
            chunk: Vec::new(),
        }
    }
}