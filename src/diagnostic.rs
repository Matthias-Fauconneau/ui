use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Code {
    pub code: String,
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpanLine {
    pub text: String,
    pub highlight_start: usize,
    pub highlight_end: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpanMacroExpansion {
    pub span: Span,
    pub macro_decl_name: String,
    pub def_site_span: Option<Span>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Span {
    pub file_name: String,
    pub byte_start: u32, pub byte_end: u32,
    pub line_start: usize, pub line_end: usize,
    pub column_start: usize, pub column_end: usize,
    pub is_primary: bool,
    pub text: Vec<SpanLine>,
    pub label: Option<String>,
    pub suggested_replacement: Option<String>,
    pub suggestion_applicability: Option<Applicability>,
    pub expansion: Option<Box<SpanMacroExpansion>>
}

#[derive(Debug, Clone, Deserialize)]
pub enum Applicability { MachineApplicable, HasPlaceholders, MaybeIncorrect, Unspecified }
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level { #[serde(rename = "error: internal compiler error")] Bug, Fatal, Error, Warning, Note, Help, Cancelled, FailureNote }

#[derive(Debug, Clone, Deserialize)]
pub struct Diagnostic {
    pub message: String,
    pub code: Option<Code>,
    pub level: Level,
    pub spans: Vec<Span>,
    pub children: Vec<Diagnostic>,
    pub rendered: Option<String>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Target {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    #[serde(rename = "required-features", default)] pub required_features: Vec<String>,
    pub src_path: PathBuf,
    pub edition: String,
    pub doctest: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactProfile {
    pub opt_level: String,
    pub debuginfo: Option<u32>,
    pub debug_assertions: bool,
    pub overflow_checks: bool,
    pub test: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Artifact {
    pub package_id: String,
    pub target: Target,
    pub profile: ArtifactProfile,
    pub features: Vec<String>,
    pub filenames: Vec<PathBuf>,
    pub executable: Option<PathBuf>,
    pub fresh: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompilerMessage {
    pub package_id: String,
    pub target: Target,
    pub message: Diagnostic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildScript {
    pub package_id: String,
    pub linked_libs: Vec<PathBuf>,
    pub linked_paths: Vec<PathBuf>,
    pub cfgs: Vec<PathBuf>,
    pub env: Vec<(String, String)>,
    pub out_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "reason", rename_all = "kebab-case")]
pub enum Message {
    CompilerArtifact(Artifact),
    CompilerMessage(CompilerMessage),
    BuildScriptExecuted(BuildScript),
}

impl std::fmt::Display for Diagnostic { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { f.write_str(&self.rendered.as_ref().ok_or(std::fmt::Error)?) } }
impl std::fmt::Display for CompilerMessage { fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result { write!(f, "{}", self.message) } }

pub fn parse<R:std::io::Read>(input: R) -> serde_json::StreamDeserializer<'static, serde_json::de::IoRead<R>, Message> {
    serde_json::Deserializer::from_reader(input).into_iter::<Message>()
}
