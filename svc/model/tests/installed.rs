use std::{env, path::PathBuf};

#[test]
#[ignore = "hashes the verified cached Stage 1 model"]
fn cached_artifact_is_reused() {
    let root = env::var_os("LAO_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("HOME").unwrap()).join("Library/Caches/lao/models")
        });
    let ready = lao_model::prepare(&root).unwrap();
    assert_eq!(ready.artifact.id, lao_model::QWEN.id);
    assert_eq!(ready.path, root.join(lao_model::QWEN.file));
}
