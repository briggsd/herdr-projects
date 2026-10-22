//! The finish line: commit, check, push, draft pull request, review, then the approval gate. Never merges by itself
//! unless `merge.by` says `itself` (finish/auto_merge.rs). Runs in a detached runner, started by the event hook when
//! herdr says the agent stopped, so it works with the board closed. One step per file in finish/.
pub mod already;
pub mod approval_kept;
pub mod auto_merge;
pub mod chat;
pub mod check_again;
pub mod check_run;
pub mod hand_over;
pub mod line;
pub mod model_refused;
pub mod old_holds;
pub mod claude_model;
pub mod closes;
pub mod compact;
pub mod pipeline;
pub mod review_pace;
pub mod review_route;
pub mod pipeline_end;
pub mod pr_body;
pub mod pr_labels;
pub mod recover_job;
pub mod research;
pub mod review;
pub mod review_once;
pub mod review_brief;
pub mod review_fallback;
pub mod shared;
pub mod sync_job;
pub mod turn_ended;
pub mod turn_hold;
pub mod waiting;
