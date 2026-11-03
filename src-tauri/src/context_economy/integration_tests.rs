use super::*;
use rusqlite::{params, Connection};
use serde_json::json;

fn fixture() -> (PathBuf, Session) {
    let data = std::env::temp_dir().join(format!("cerne-rust-economy-{}", uuid::Uuid::new_v4()));
    let project = data.join("project with spaces");
    std::fs::create_dir_all(&project).unwrap();
    let session = crate::sessions::create_session(
        &data,
        "Native economy".into(),
        crate::models::ProviderKind::Custom,
        "gpt-4o".into(),
        Some(project.to_string_lossy().into_owned()),
        Some(70000),
        None,
        None,
        None,
    )
    .unwrap();
    (data, session)
}
fn message(role: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: role.into(),
        content: content.into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
        images: vec![],
        display_content: None,
    }
}

#[tokio::test]
async fn manual_compaction_works_below_the_automatic_threshold() {
    let (data, session)=fixture();let clients=McpClients::default();
    let mut messages=vec![message("system","System"),message("user","Quero concluir manualgoal751 sem apagar o historico.")];
    for _ in 0..24 {messages.push(message("assistant",&"Resultado longo ação 中文.\n".repeat(200)));}
    crate::sessions::save_messages(&data,&session.id,&messages).unwrap();
    sync(&data,&session.id,&clients).await.unwrap();
    let (automatic,changed)=prepare_view(&data,&session.id,&messages,&[],"gpt-4o",1_000_000,false,&clients).await.unwrap();
    assert!(!changed);assert_eq!(automatic.len(),messages.len());
    let (manual,changed)=prepare_view(&data,&session.id,&messages,&[],"gpt-4o",1_000_000,true,&clients).await.unwrap();
    assert!(changed);assert!(manual.len()<messages.len());
    assert_eq!(crate::sessions::load_messages(&data,&session.id).unwrap().len(),messages.len());
}

#[tokio::test]
async fn storage_failure_never_produces_a_reduced_view_or_overwrites_history() {
    let (data,session)=fixture();let clients=McpClients::default();
    let raw=format!("{}storagefailure751 ação 中文", "complete output\n".repeat(3000));
    let messages=vec![message("system","System"),message("user","Preserve todos os dados"),message("assistant",&raw)];
    crate::sessions::save_messages(&data,&session.id,&messages).unwrap();
    let original=std::fs::read(data.join("sessions").join(&session.id).join("chat_log.json")).unwrap();
    // An unopenable database simulates unavailable persistence without changing
    // permissions or deleting any files outside this owned fixture.
    let root=dir(&data,&session.id);std::fs::create_dir_all(root.join("native.sqlite3")).unwrap();
    assert!(archive_output(&data,&session.id,"run_command","failed-output",&raw,&clients).await.is_err());
    assert!(sync(&data,&session.id,&clients).await.is_err());
    assert_eq!(std::fs::read(data.join("sessions").join(&session.id).join("chat_log.json")).unwrap(),original);
    assert!(!root.join("view.json").exists());
    assert_eq!(saved_view(&data,&session.id,&messages).unwrap().len(),messages.len());
}
#[test]
fn fts_fusion_typos_filters_and_unicode() {
    let (data, session) = fixture();
    let mut s = store::Store::open(&data, &session.id).unwrap();
    s.index(
        "# useEffect\n```js\nconst cache = caching();\n```\nAção 中文 decisaorust751 preservar.",
        "unicode-spec",
        None,
    )
    .unwrap();
    s.index("# unrelated\nNothing useful here", "other-source", None)
        .unwrap();
    assert!(s
        .search("useEff", Some("unicode"), Some("code"), 3, false)
        .unwrap()[0]
        .content
        .contains("caching"));
    assert!(!s
        .search("cached", Some("unicode"), None, 3, false)
        .unwrap()
        .is_empty());
    assert!(!s
        .search("decisaorust752", Some("unicode"), None, 3, false)
        .unwrap()
        .is_empty());
    assert!(
        s.search("中文", Some("unicode"), None, 3, false).unwrap()[0]
            .content
            .contains("中文")
    );
    assert!(s
        .search("decisaorust751", Some("other"), None, 3, false)
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn all_native_tools_have_a_contract_and_slash_aliases() {
    let (data, session) = fixture();
    let specs = tools::specs(&session.id).unwrap();
    assert_eq!(specs.len(), 10);
    assert!(specs.iter().all(|s| s.function.name.len() <= 64));
    assert!(specs
        .iter()
        .all(|s| s.function.parameters["type"] == "object"));
    assert!(tools::command(&data, &session, "/context-mode:ctx-doctor")
        .await
        .unwrap()
        .contains("Rust nativo: OK"));
    assert!(tools::command(&data, &session, "/ctx-stats")
        .await
        .unwrap()
        .contains("stored_tokens_estimate"));
    assert!(tools::command(&data, &session, "/ctx-upgrade")
        .await
        .unwrap()
        .contains("nova versao do Cerne"));
    assert_eq!(
        tools::slash("/ctx-search decisão guardada"),
        Some(("ctx-search", "decisão guardada"))
    );
    assert!(tools::slash("/ctx-searching whatever").is_none());
}
#[tokio::test]
async fn automatic_compaction_archives_every_byte_and_survives_reopening() {
    let (data, session) = fixture();
    let clients = McpClients::default();
    let mut messages = vec![
        message("system", "System"),
        message(
            "user",
            "Nunca apague os arquivos. Quero portar o contexto para Rust. objetivoeconomia123",
        ),
    ];
    for i in 0..24 {
        messages.push(message("user", &format!("Pedido {i}")));
        messages.push(message(
            "assistant",
            &format!(
                "Turno {i}: {}",
                "Ação 中文 resultado completo.\n".repeat(200)
            ),
        ));
    }
    crate::sessions::save_messages(&data, &session.id, &messages).unwrap();
    sync(&data, &session.id, &clients).await.unwrap();
    sync(&data, &session.id, &clients).await.unwrap();
    let records = store::Store::open(&data, &session.id)
        .unwrap()
        .stats()
        .unwrap()["records"]
        .clone();
    sync(&data, &session.id, &clients).await.unwrap();
    assert_eq!(
        store::Store::open(&data, &session.id)
            .unwrap()
            .stats()
            .unwrap()["records"],
        records
    );
    let (small, compacted) = prepare_view(
        &data,
        &session.id,
        &messages,
        &[],
        "gpt-4o",
        10000,
        false,
        &clients,
    )
    .await
    .unwrap();
    assert!(compacted);
    assert!(small.len() < messages.len());
    assert!(small[1].content.contains("objetivoeconomia123"));
    assert_eq!(
        crate::sessions::load_messages(&data, &session.id)
            .unwrap()
            .len(),
        messages.len()
    );
    assert_eq!(
        saved_view(&data, &session.id, &messages).unwrap().len(),
        small.len()
    );
    let raw = format!(
        "{}\nErro raracerne472 encoding ISO-8859-1 ação 中文",
        "INFO normal\n".repeat(4000)
    );
    let reduced = archive_output(
        &data,
        &session.id,
        "run_command",
        "output-1",
        &raw,
        &clients,
    )
    .await
    .unwrap();
    assert!(reduced.len() < raw.len() / 10);
    let s = store::Store::open(&data, &session.id).unwrap();
    let saved: String =
        s.db.query_row(
            "SELECT raw FROM sources WHERE label='tool-output-output-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(saved, raw);
    assert!(s
        .search("raracerne472", Some("tool-output-output-1"), None, 3, false)
        .unwrap()[0]
        .content
        .contains("中文"));
    drop(s);
    reset_view(&data, &session.id).unwrap();
    assert_eq!(
        saved_view(&data, &session.id, &messages).unwrap().len(),
        messages.len()
    );
    assert!(store::Store::open(&data, &session.id)
        .unwrap()
        .search("raracerne472", None, None, 3, false)
        .unwrap()[0]
        .content
        .contains("ISO-8859-1"));
}
#[tokio::test]
async fn below_threshold_does_not_compact_or_modify_transcript() {
    let (data, session) = fixture();
    let messages = vec![message("system", "system"), message("user", "hello")];
    let clients = McpClients::default();
    let (view, changed) = prepare_view(
        &data,
        &session.id,
        &messages,
        &[],
        "gpt-4o",
        70000,
        false,
        &clients,
    )
    .await
    .unwrap();
    assert!(!changed);
    assert_eq!(
        serde_json::to_string(&view).unwrap(),
        serde_json::to_string(&messages).unwrap()
    );
}
#[tokio::test]
async fn native_file_execution_and_batch_keep_context_small() {
    let (data, session) = fixture();
    let project = PathBuf::from(session.project_root.as_ref().unwrap());
    std::fs::write(
        project.join("huge.log"),
        format!("{}ERROR rarecerne598", "INFO ok\n".repeat(4000)),
    )
    .unwrap();
    let result=tools::execute_tool(&data,&session,"ctx_execute_file",&json!({"path":"huge.log","language":"javascript","code":"console.log(FILE_CONTENT.split('\\n').filter(x=>x.includes('ERROR')).join('\\n'))"}),None).await.unwrap();
    assert!(result.contains("rarecerne598"));
    assert!(result.len() < 1000);
    let result=tools::execute_tool(&data,&session,"ctx_batch_execute",&json!({"commands":[{"label":"shell","command":"echo rarebatch123"}],"queries":["rarebatch123"]}),None).await.unwrap();
    assert!(result.contains("rarebatch123"));
}
#[tokio::test]
async fn purge_and_isolation_cannot_touch_another_chat() {
    let (data, a) = fixture();
    let (_, b) = fixture();
    let mut b = b;
    b.project_root = a.project_root.clone();
    let mut s = store::Store::open(&data, &a.id).unwrap();
    s.index("privatea123 keep", "private-a", None).unwrap();
    drop(s);
    let mut s = store::Store::open(&data, &b.id).unwrap();
    s.index("privateb456 keep", "private-b", None).unwrap();
    drop(s);
    assert!(tools::execute_tool(
        &data,
        &a,
        "ctx_purge",
        &json!({"confirm":true,"sessionId":b.id}),
        None
    )
    .await
    .is_err());
    let dry = tools::command(&data, &a, "/ctx-purge").await.unwrap();
    assert!(dry.contains("confirme"));
    assert!(!store::Store::open(&data, &a.id)
        .unwrap()
        .search("privatea123", None, None, 3, false)
        .unwrap()
        .is_empty());
    tools::command(&data, &a, "/ctx-purge --confirm")
        .await
        .unwrap();
    assert!(store::Store::open(&data, &a.id)
        .unwrap()
        .search("privatea123", None, None, 3, false)
        .unwrap()
        .is_empty());
    assert!(!store::Store::open(&data, &b.id)
        .unwrap()
        .search("privateb456", None, None, 3, false)
        .unwrap()
        .is_empty());
}
#[test]
fn legacy_migration_is_read_only_and_idempotent() {
    let (data, session) = fixture();
    let root = dir(&data, &session.id);
    std::fs::create_dir_all(root.join("content")).unwrap();
    let old = root.join("content/old.db");
    let db = Connection::open(&old).unwrap();
    db.execute_batch("CREATE TABLE sources(id INTEGER PRIMARY KEY,label TEXT);CREATE TABLE chunks(title TEXT,content TEXT,source_id INTEGER);INSERT INTO sources VALUES (1,'legacy-log');").unwrap();
    db.execute(
        "INSERT INTO chunks VALUES ('error',?,1)",
        params!["migracaorust456 ação 中文 keep"],
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&old).unwrap();
    let mut s = store::Store::open(&data, &session.id).unwrap();
    assert_eq!(s.import_legacy().unwrap(), 1);
    assert_eq!(s.import_legacy().unwrap(), 0);
    assert_eq!(std::fs::read(&old).unwrap(), before);
    assert!(s
        .search("migracaorust456", Some("legacy-log"), None, 3, false)
        .unwrap()[0]
        .content
        .contains("中文"));
}
#[tokio::test]
async fn directory_limits_paths_and_throttling_are_enforced() {
    let (data, session) = fixture();
    let project = PathBuf::from(session.project_root.as_ref().unwrap());
    for i in 0..5 {
        std::fs::write(
            project.join(format!("{i}.md")),
            format!("indexrust452 arquivo {i}"),
        )
        .unwrap();
    }
    let result = tools::execute_tool(
        &data,
        &session,
        "ctx_index",
        &json!({"path":".","maxFiles":2}),
        None,
    )
    .await
    .unwrap();
    assert!(result.contains("Indexed 2 files"));
    assert!(tools::execute_tool(&data,&session,"ctx_execute_file",&json!({"path":"../native.sqlite3","language":"javascript","code":"console.log(FILE_CONTENT)"}),None).await.is_err());
    for _ in 0..8 {
        tools::execute_tool(
            &data,
            &session,
            "ctx_search",
            &json!({"queries":["indexrust452"]}),
            None,
        )
        .await
        .unwrap();
    }
    assert!(tools::execute_tool(
        &data,
        &session,
        "ctx_search",
        &json!({"queries":["indexrust452"]}),
        None
    )
    .await
    .unwrap_err()
    .to_string()
    .contains("Limite de busca"));
}

#[tokio::test]
async fn changed_authorized_files_refresh_before_search() {
    let (data, session) = fixture();
    let project = PathBuf::from(session.project_root.as_ref().unwrap());
    std::fs::write(project.join("state.md"), "previousstate456").unwrap();
    tools::execute_tool(
        &data,
        &session,
        "ctx_index",
        &json!({"path":"state.md"}),
        None,
    )
    .await
    .unwrap();
    std::fs::write(project.join("state.md"), "updatedstate789 ação 中文").unwrap();
    let result = tools::execute_tool(
        &data,
        &session,
        "ctx_search",
        &json!({"queries":["updatedstate789"]}),
        None,
    )
    .await
    .unwrap();
    assert!(result.contains("updatedstate789"));
    assert!(result.contains("中文"));
    assert!(store::Store::open(&data, &session.id)
        .unwrap()
        .search("previousstate456", None, None, 3, false)
        .unwrap()
        .is_empty());
}
