use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::Result;
use secall_core::{
    command_exists,
    store::{get_default_db_path, Database},
    vault::{git::VaultGit, Config, Vault},
};

pub fn run(vault: Option<PathBuf>, git: Option<String>) -> Result<()> {
    if vault.is_some() || git.is_some() {
        return run_non_interactive(vault, git);
    }
    run_interactive()
}

fn run_non_interactive(vault: Option<PathBuf>, git: Option<String>) -> Result<()> {
    let mut config = Config::load_or_default();

    if let Some(v) = vault {
        config.vault.path = v;
    }

    let vault_path = config.vault.path.clone();
    println!("Initializing seCall...");
    println!("  Vault:  {}", vault_path.display());
    println!("  Config: {}", Config::config_path().display());
    println!("  DB:     {}", get_default_db_path().display());

    config.save()?;

    let v = Vault::new(vault_path.clone());
    v.init()?;

    let db_path = get_default_db_path();
    let _ = Database::open(&db_path)?;

    if let Some(remote) = git {
        let vault_git = VaultGit::new(&vault_path, &config.vault.branch);
        vault_git.init(&remote)?;
        config.vault.git_remote = Some(remote);
        config.save()?;
        println!("Git remote configured. Use `secall sync` to push/pull.");
    }

    print_completion_hints();
    Ok(())
}

fn run_interactive() -> Result<()> {
    use dialoguer::{Input, Select};

    println!();
    println!("  seCall — Agent Session Search Engine");
    println!("  =====================================");
    println!();

    let mut config = Config::load_or_default();

    // Step 1: Vault 경로
    println!("  Step 1/6: Vault 경로");
    println!("  Obsidian vault 경로를 입력하세요");
    let vault_default = config.vault.path.to_string_lossy().to_string();
    let vault_input: String = Input::new()
        .with_prompt("  >")
        .default(vault_default)
        .interact_text()?;
    let vault_path = PathBuf::from(shellexpand::tilde(&vault_input).to_string());
    config.vault.path = vault_path.clone();
    println!();

    // Step 2: Git remote
    println!("  Step 2/6: Git 동기화 (선택)");
    println!("  멀티 기기 동기화를 위한 Git remote URL");
    println!("  없으면 Enter를 누르세요");
    let git_default = config.vault.git_remote.clone().unwrap_or_default();
    let git_input: String = Input::new()
        .with_prompt("  >")
        .default(git_default)
        .allow_empty(true)
        .interact_text()?;
    let git_remote = if git_input.trim().is_empty() {
        None
    } else {
        Some(git_input.trim().to_string())
    };
    config.vault.git_remote = git_remote.clone();
    println!();

    // Step 3: Git 브랜치 (git remote 설정 시만 표시)
    if git_remote.is_some() {
        println!("  Step 3/6: Git 브랜치");
        let branch_input: String = Input::new()
            .with_prompt("  >")
            .default(config.vault.branch.clone())
            .interact_text()?;
        config.vault.branch = branch_input.trim().to_string();
        println!();
    }

    // Step 4: 토크나이저
    println!("  Step 4/6: 토크나이저");
    #[cfg(not(target_os = "windows"))]
    let tokenizer_items = vec![
        "lindera — 한국어+일본어 형태소 분석 (범용)",
        "kiwi — 한국어 전용, 더 정확 (macOS/Linux만 지원)",
    ];
    #[cfg(target_os = "windows")]
    let tokenizer_items = vec!["lindera — 한국어+일본어 형태소 분석 (범용)"];

    let tokenizer_default = if config.search.tokenizer == "kiwi" {
        #[cfg(not(target_os = "windows"))]
        {
            1usize
        }
        #[cfg(target_os = "windows")]
        {
            0usize
        }
    } else {
        0usize
    };

    let tokenizer_sel = Select::new()
        .with_prompt("  선택")
        .items(&tokenizer_items)
        .default(tokenizer_default)
        .interact()?;

    #[cfg(not(target_os = "windows"))]
    {
        config.search.tokenizer = if tokenizer_sel == 1 {
            "kiwi".to_string()
        } else {
            "lindera".to_string()
        };
    }
    #[cfg(target_os = "windows")]
    {
        let _ = tokenizer_sel;
        config.search.tokenizer = "lindera".to_string();
    }
    println!();

    // Step 5: 임베딩 백엔드
    println!("  Step 5/6: 임베딩 백엔드");
    let backend_items = vec![
        "ollama — 로컬 임베딩 (bge-m3, 무료)",
        "none — 벡터 검색 비활성화 (BM25만 사용)",
    ];
    let backend_default = if config.embedding.backend == "none" {
        1usize
    } else {
        0usize
    };
    let backend_sel = Select::new()
        .with_prompt("  선택")
        .items(&backend_items)
        .default(backend_default)
        .interact()?;
    config.embedding.backend = if backend_sel == 0 {
        "ollama".to_string()
    } else {
        "none".to_string()
    };
    // BM25-only 선택 시 시맨틱 그래프 추출도 비활성화
    if config.embedding.backend == "none" {
        config.graph.semantic = false;
        println!("  → BM25만 사용: 시맨틱 그래프 추출도 비활성화됩니다.");
    }
    println!();

    // Step 6: Ollama 확인 (ollama 선택 시만)
    if config.embedding.backend == "ollama" {
        println!("  Step 6/6: Ollama 설정");
        check_and_setup_ollama()?;
        println!();
    }

    // 설정 저장
    config.save()?;
    println!("  Config: {}", Config::config_path().display());

    // Vault 초기화
    let v = Vault::new(vault_path.clone());
    v.init()?;
    println!("  Vault:  {}", vault_path.display());

    // DB 초기화
    let db_path = get_default_db_path();
    let _ = Database::open(&db_path)?;
    println!("  DB:     {}", db_path.display());

    // Git 초기화
    if let Some(remote) = &git_remote {
        let vault_git = VaultGit::new(&vault_path, &config.vault.branch);
        vault_git.init(remote)?;
        println!("  Git remote configured. Use `secall sync` to push/pull.");
    }

    println!();
    println!("  ✓ 초기화 완료.");
    print_completion_hints();
    Ok(())
}

fn check_and_setup_ollama() -> Result<()> {
    if !command_exists("ollama") {
        println!("  ✗ Ollama가 설치되어 있지 않습니다.");
        println!("    설치 방법:");
        println!("    macOS:   brew install ollama");
        println!("    Linux:   curl -fsSL https://ollama.com/install.sh | sh");
        println!("    Windows: https://ollama.com/download");
        println!();
        println!("    설치 후 다음 명령을 실행하세요:");
        println!("      ollama serve          # 서버 시작");
        println!("      ollama pull bge-m3    # 임베딩 모델 다운로드");
        println!();
        println!("    Ollama 없이도 BM25 검색은 사용 가능합니다.");
        println!("    나중에 `secall config set embedding.backend ollama`로 변경할 수 있습니다.");
        return Ok(());
    }
    println!("  ✓ Ollama가 설치되어 있습니다.");

    // ollama 서버 동작 확인
    let list_output = Command::new("ollama").args(["list"]).output();
    let list_output = match list_output {
        Ok(o) if o.status.success() => o,
        _ => {
            println!("  ⚠ Ollama 서버가 실행 중이 아닙니다.");
            println!("    `ollama serve`로 서버를 시작한 후 다시 시도하세요.");
            return Ok(());
        }
    };

    let models = String::from_utf8_lossy(&list_output.stdout);
    if models.contains("bge-m3") {
        println!("  ✓ bge-m3 모델이 이미 설치되어 있습니다.");
        return Ok(());
    }

    println!("  ⟳ ollama pull bge-m3 실행 중... (최초 ~1.5GB 다운로드)");
    let status = Command::new("ollama")
        .args(["pull", "bge-m3"])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() {
        println!("  ✓ bge-m3 모델 준비 완료.");
    } else {
        println!("  ⚠ 모델 다운로드 실패. 나중에 `ollama pull bge-m3`로 재시도하세요.");
    }
    Ok(())
}

fn print_completion_hints() {
    println!("\n✓ Initialization complete.");
    println!("\nTo configure Claude Code for auto-ingest, add to ~/.claude/settings.json:");
    println!(
        r#"{{
  "hooks": {{
    "SessionEnd": [{{
      "hooks": [{{"type": "command", "command": "secall ingest --auto --cwd $PWD"}}]
    }}]
  }}
}}"#
    );
    println!("\nTo start MCP server, add to ~/.claude/settings.json:");
    println!(
        r#"{{
  "mcpServers": {{
    "secall": {{
      "command": "secall",
      "args": ["mcp"]
    }}
  }}
}}"#
    );
}
