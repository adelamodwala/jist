use json_tools::TokenType;
use log::debug;
use crate::utils::{array_ind, checkpoint_depth};

pub(crate) struct JStructTracker {
    // tuple of (depth, arr_depth, obj_depth)
    pub depth_curr: (i32, i32, i32),

    // keep track of array indices if currently inside array
    pub arr_idx: Vec<i64>,
    pub last_open: Vec<TokenType>,
    pub checkpoint_start: Vec<u64>,
    pub last_token_key_delimiter: bool,

    // build checkpoints that must pass
    pub checkpoints: Vec<(i32, i32, i32)>,
    pub search_keys: Vec<String>,
    pub arr_tgt: Vec<i64>,
    pub arr_tgt_size: usize,
}
impl JStructTracker {

    pub fn init() -> JStructTracker {
        JStructTracker {
            depth_curr: (-1, -1, -1),
            arr_idx: Vec::new(),
            last_open: Vec::new(),
            checkpoint_start: Vec::new(),
            last_token_key_delimiter: false,
            checkpoints: Vec::new(),
            search_keys: Vec::new(),
            arr_tgt: Vec::new(),
            arr_tgt_size: 0,
        }
    }
    pub fn new(search_path: &[String]) -> JStructTracker {
        let mut struct_tracker = Self::init();

        for search_idx in 0..search_path.len() {
            struct_tracker
                .checkpoints
                .push(checkpoint_depth(search_path, search_idx));
        }
        struct_tracker.checkpoints.reverse();

        struct_tracker.arr_tgt = search_path
            .iter()
            .filter(|x| x.starts_with('['))
            .map(|x| array_ind(x.as_str()))
            .rev()
            .collect();
        struct_tracker.arr_tgt_size = struct_tracker.arr_tgt.len();
        struct_tracker.search_keys = search_path
            .iter()
            .filter(|x| !x.starts_with('['))
            .cloned()
            .rev()
            .collect::<Vec<String>>();
        debug!(
            "checkpoints: {:?}, arr_tgt: {:?}, search_keys: {:?}",
            struct_tracker.checkpoints, struct_tracker.arr_tgt, struct_tracker.search_keys
        );
        struct_tracker
    }
}