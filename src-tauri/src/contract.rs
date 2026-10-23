//! Launcher <-> game contract v0 (see `docs/contract.md`).
//!
//! Everything the launcher reads from or writes to the game side is typed here: the version
//! manifest, the extractor's JSON-lines protocol and exit codes, `extract-info.json`, and the
//! runtime's `--version-json` answer. The TypeScript mirror is `src/backend/contract.ts`; keep
//! both in step with the doc.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

/// Manifest schema this launcher understands.
pub const MANIFEST_SCHEMA: u32 = 1;
/// File name of the manifest at the root of every version folder.
pub const MANIFEST_FILE: &str = "rerac-manifest.json";
/// File the extractor writes into the game data dir on success.
pub const EXTRACT_INFO_FILE: &str = "extract-info.json";
/// Env var equivalent of `--data-dir` for the runtime.
pub const DATA_DIR_ENV: &str = "RC_DATA_DIR";

/// `rerac-manifest.json` at the root of `versions/<source>/<version>/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: u32,
    pub name: String,
    pub version: String,
    pub game: String,
    /// Path of the game binary, relative to the version folder.
    pub runtime: String,
    /// Path of the extractor binary, relative to the version folder.
    pub extractor: String,
    pub supported_discs: Vec<String>,
    pub data_format: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Json(String),
    Schema(u32),
    Name(String),
    Field(&'static str),
    UnsafePath(&'static str, String),
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::Json(e) => write!(f, "{MANIFEST_FILE} is not valid: {e}"),
            ManifestError::Schema(s) => write!(
                f,
                "{MANIFEST_FILE} uses schema {s}; this launcher understands schema {MANIFEST_SCHEMA}. Update the launcher."
            ),
            ManifestError::Name(n) => {
                write!(f, "{MANIFEST_FILE} is for \"{n}\", not rerac")
            }
            ManifestError::Field(k) => write!(f, "{MANIFEST_FILE}: \"{k}\" is empty"),
            ManifestError::UnsafePath(k, p) => write!(
                f,
                "{MANIFEST_FILE}: \"{k}\" must be a relative path inside the version folder (got \"{p}\")"
            ),
        }
    }
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
        let m: Manifest = serde_json::from_str(text).map_err(|e| ManifestError::Json(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.schema != MANIFEST_SCHEMA {
            return Err(ManifestError::Schema(self.schema));
        }
        if self.name != "rerac" {
            return Err(ManifestError::Name(self.name.clone()));
        }
        for (key, value) in [("version", &self.version), ("game", &self.game)] {
            if value.trim().is_empty() {
                return Err(ManifestError::Field(key));
            }
        }
        for (key, value) in [("runtime", &self.runtime), ("extractor", &self.extractor)] {
            if value.trim().is_empty() {
                return Err(ManifestError::Field(key));
            }
            if !is_contained_relative(value) {
                return Err(ManifestError::UnsafePath(key, value.clone()));
            }
        }
        Ok(())
    }
}

/// True for a relative path that cannot climb out of its base folder.
fn is_contained_relative(p: &str) -> bool {
    let path = Path::new(p);
    !path.is_absolute()
        && path
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

/// Extraction stage reported by `progress` lines. `extract` ends with `prepare` (the engine cache);
/// `export` (the optional Tier 2 exports, game-side clarification 18) reports `export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Identify,
    Copy,
    Verify,
    Prepare,
    Export,
}

/// One JSON line on the extractor's stdout with `--json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ExtractorEvent {
    Progress {
        stage: Stage,
        done: u64,
        total: u64,
        #[serde(default)]
        file: String,
    },
    Info {
        message: String,
    },
    /// Emitted whenever the boot ELF was read: on success, for code 21, and for code 20 when the
    /// image is another PS2 game. `game`, `title` and `elf_sha1` are the game side's optional
    /// extras (clarification 2); `game` is rac1, rac2, rac3, racdl or unknown.
    Disc {
        serial: String,
        region: String,
        version: String,
        supported: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        game: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elf_sha1: Option<String>,
    },
    Error {
        code: i32,
        message: String,
    },
    Done {
        elapsed_ms: u64,
    },
}

/// Parses one stdout line. Blank lines give `None`; anything that is not a contract object is
/// surfaced as an `info` event so nothing the extractor prints is silently lost.
pub fn parse_event_line(line: &str) -> Option<ExtractorEvent> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    match serde_json::from_str::<ExtractorEvent>(line) {
        Ok(ev) => Some(ev),
        Err(_) => Some(ExtractorEvent::Info {
            message: line.to_string(),
        }),
    }
}

/// Extractor error codes. The process exit code equals the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Ok,
    CannotRead,
    NotIso9660,
    UnknownDisc,
    UnsupportedBuild,
    WriteFailure,
    DiskFull,
    VerifyFailed,
    Internal,
}

impl ErrorCode {
    pub const ALL: [ErrorCode; 9] = [
        ErrorCode::Ok,
        ErrorCode::CannotRead,
        ErrorCode::NotIso9660,
        ErrorCode::UnknownDisc,
        ErrorCode::UnsupportedBuild,
        ErrorCode::WriteFailure,
        ErrorCode::DiskFull,
        ErrorCode::VerifyFailed,
        ErrorCode::Internal,
    ];

    pub fn code(self) -> i32 {
        match self {
            ErrorCode::Ok => 0,
            ErrorCode::CannotRead => 10,
            ErrorCode::NotIso9660 => 11,
            ErrorCode::UnknownDisc => 20,
            ErrorCode::UnsupportedBuild => 21,
            ErrorCode::WriteFailure => 30,
            ErrorCode::DiskFull => 31,
            ErrorCode::VerifyFailed => 40,
            ErrorCode::Internal => 99,
        }
    }

    /// Unknown codes map to `Internal`, matching how the UI treats them.
    pub fn from_code(code: i32) -> ErrorCode {
        ErrorCode::ALL
            .into_iter()
            .find(|c| c.code() == code)
            .unwrap_or(ErrorCode::Internal)
    }
}

/// `<game_data_dir>/extract-info.json`, written by the extractor on success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractInfo {
    pub disc: String,
    pub data_format: u32,
    pub extractor_version: String,
    /// The PAL copies were skipped (`--ntsc-only`, R&C1 only). Optional: a game whose extractor
    /// never skips anything (Going Commando) may leave it out, which means `false`.
    #[serde(default)]
    pub ntsc_only: bool,
    pub files: u64,
    pub bytes: u64,
}

impl ExtractInfo {
    pub fn parse(text: &str) -> Result<ExtractInfo, String> {
        serde_json::from_str(text).map_err(|e| format!("{EXTRACT_INFO_FILE} is not valid: {e}"))
    }
}

/// `<runtime> --version-json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
    pub version: String,
    pub game: String,
    pub data_format: u32,
}

impl RuntimeVersion {
    /// The runtime may print log noise before the JSON; take the last line that parses.
    pub fn parse(stdout: &str) -> Result<RuntimeVersion, String> {
        stdout
            .lines()
            .rev()
            .find_map(|l| serde_json::from_str::<RuntimeVersion>(l.trim()).ok())
            .ok_or_else(|| "the runtime did not print a --version-json object".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"schema":1,"name":"rerac","version":"0.1.0","game":"rac1","runtime":"bin/rerac","extractor":"bin/rerac-extract","supported_discs":["SCUS_971.99"],"data_format":1}"#;

    #[test]
    fn manifest_parses_contract_example() {
        let m = Manifest::parse(GOOD).unwrap();
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.game, "rac1");
        assert_eq!(m.runtime, "bin/rerac");
        assert_eq!(m.supported_discs, vec!["SCUS_971.99".to_string()]);
        assert_eq!(m.data_format, 1);
    }

    #[test]
    fn manifest_rejects_wrong_schema_name_and_paths() {
        let bad_schema = GOOD.replace(r#""schema":1"#, r#""schema":2"#);
        assert_eq!(Manifest::parse(&bad_schema), Err(ManifestError::Schema(2)));
        let bad_name = GOOD.replace(r#""name":"rerac""#, r#""name":"other""#);
        assert!(matches!(Manifest::parse(&bad_name), Err(ManifestError::Name(_))));
        let escape = GOOD.replace("bin/rerac-extract", "../../evil");
        assert!(matches!(
            Manifest::parse(&escape),
            Err(ManifestError::UnsafePath("extractor", _))
        ));
        let abs = GOOD.replace("bin/rerac\"", "/usr/bin/rerac\"");
        assert!(matches!(
            Manifest::parse(&abs),
            Err(ManifestError::UnsafePath("runtime", _))
        ));
        let missing = r#"{"schema":1,"name":"rerac"}"#;
        assert!(matches!(Manifest::parse(missing), Err(ManifestError::Json(_))));
    }

    #[test]
    fn parses_every_event_kind() {
        assert_eq!(
            parse_event_line(r#"{"type":"progress","stage":"copy","done":10,"total":100,"file":"LEVEL/01.WAD"}"#),
            Some(ExtractorEvent::Progress {
                stage: Stage::Copy,
                done: 10,
                total: 100,
                file: "LEVEL/01.WAD".into()
            })
        );
        assert_eq!(
            parse_event_line(r#"{"type":"info","message":"hello"}"#),
            Some(ExtractorEvent::Info { message: "hello".into() })
        );
        assert_eq!(
            parse_event_line(
                r#"{"type":"disc","serial":"SCUS_971.99","region":"NTSC-U","version":"1.00","supported":true}"#
            ),
            Some(ExtractorEvent::Disc {
                serial: "SCUS_971.99".into(),
                region: "NTSC-U".into(),
                version: "1.00".into(),
                supported: true,
                game: None,
                title: None,
                elf_sha1: None,
            })
        );
        // With the game side's extras (clarification 2).
        assert_eq!(
            parse_event_line(
                r#"{"type":"disc","serial":"SCUS_972.68","region":"NTSC-U","version":"1.00","supported":false,"game":"rac2","title":"Ratchet & Clank: Going Commando","elf_sha1":"ab12"}"#
            ),
            Some(ExtractorEvent::Disc {
                serial: "SCUS_972.68".into(),
                region: "NTSC-U".into(),
                version: "1.00".into(),
                supported: false,
                game: Some("rac2".into()),
                title: Some("Ratchet & Clank: Going Commando".into()),
                elf_sha1: Some("ab12".into()),
            })
        );
        assert_eq!(
            parse_event_line(r#"{"type":"error","code":21,"message":"PAL"}"#),
            Some(ExtractorEvent::Error { code: 21, message: "PAL".into() })
        );
        assert_eq!(
            parse_event_line(r#"{"type":"done","elapsed_ms":1234}"#),
            Some(ExtractorEvent::Done { elapsed_ms: 1234 })
        );
    }

    #[test]
    fn progress_without_file_and_unknown_fields_are_accepted() {
        let ev = parse_event_line(r#"{"type":"progress","stage":"verify","done":1,"total":2,"extra":true}"#);
        assert_eq!(
            ev,
            Some(ExtractorEvent::Progress { stage: Stage::Verify, done: 1, total: 2, file: String::new() })
        );
    }

    #[test]
    fn export_progress_lines_parse() {
        let ev = parse_event_line(r#"{"type":"progress","stage":"export","done":5,"total":9,"file":"levels/01/core_data.bin"}"#);
        assert_eq!(
            ev,
            Some(ExtractorEvent::Progress { stage: Stage::Export, done: 5, total: 9, file: "levels/01/core_data.bin".into() })
        );
    }

    #[test]
    fn non_contract_lines_become_info_and_blank_lines_vanish() {
        assert_eq!(parse_event_line("   "), None);
        assert_eq!(
            parse_event_line("warning: something"),
            Some(ExtractorEvent::Info { message: "warning: something".into() })
        );
        assert_eq!(
            parse_event_line(r#"{"type":"mystery"}"#),
            Some(ExtractorEvent::Info { message: r#"{"type":"mystery"}"#.into() })
        );
    }

    #[test]
    fn error_codes_round_trip() {
        let expected = [0, 10, 11, 20, 21, 30, 31, 40, 99];
        for (c, n) in ErrorCode::ALL.iter().zip(expected) {
            assert_eq!(c.code(), n);
            assert_eq!(ErrorCode::from_code(n), *c);
        }
        assert_eq!(ErrorCode::from_code(12345), ErrorCode::Internal);
        assert_eq!(ErrorCode::from_code(-1), ErrorCode::Internal);
    }

    #[test]
    fn extract_info_and_runtime_version_parse() {
        let info = ExtractInfo::parse(
            r#"{"disc":"SCUS_971.99","data_format":1,"extractor_version":"0.1.0","ntsc_only":false,"files":812,"bytes":4000000000}"#,
        )
        .unwrap();
        assert_eq!(info.files, 812);
        assert_eq!(info.bytes, 4_000_000_000);
        // Going Commando's receipt: no ntsc_only (nothing is ever skipped), plus its own `game` key.
        let rac2 = ExtractInfo::parse(
            r#"{"game":"rac2","disc":"SCUS_972.68","data_format":1,"extractor_version":"0.1.1","files":96,"bytes":3807638923}"#,
        )
        .unwrap();
        assert!(!rac2.ntsc_only);
        assert_eq!(rac2.disc, "SCUS_972.68");
        let rv = RuntimeVersion::parse(
            "starting up\n{\"name\":\"rerac\",\"version\":\"0.1.0\",\"game\":\"rac1\",\"data_format\":1}\n",
        )
        .unwrap();
        assert_eq!(rv.version, "0.1.0");
        assert!(RuntimeVersion::parse("nothing here").is_err());
    }
}
