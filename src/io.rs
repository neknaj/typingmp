extern crate alloc;

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
use alloc::format;
use alloc::{
    borrow::Cow,
    string::{String, ToString},
    vec::Vec,
};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub message: String,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

#[cfg(not(feature = "uefi"))]
impl std::error::Error for ProviderError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderErrorKind {
    NotFound,
    InvalidId,
    Unsupported,
    Io,
    Decode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomProblem {
    pub name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProblemSourceKind {
    Builtin,
    Custom,
    OpenFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProblemId {
    pub kind: ProblemSourceKind,
    pub index: usize,
}

impl ProblemId {
    pub const fn builtin(index: usize) -> Self {
        Self {
            kind: ProblemSourceKind::Builtin,
            index,
        }
    }

    pub const fn custom(index: usize) -> Self {
        Self {
            kind: ProblemSourceKind::Custom,
            index,
        }
    }

    pub const fn open_file(index: usize) -> Self {
        Self {
            kind: ProblemSourceKind::OpenFile,
            index,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProblemCapabilities {
    pub can_start: bool,
    pub can_view_source: bool,
    pub can_delete: bool,
    pub can_move: bool,
    pub can_open_file: bool,
}

impl ProblemCapabilities {
    pub const BUILTIN: Self = Self {
        can_start: true,
        can_view_source: true,
        can_delete: false,
        can_move: false,
        can_open_file: false,
    };

    pub const CUSTOM: Self = Self {
        can_start: true,
        can_view_source: true,
        can_delete: true,
        can_move: true,
        can_open_file: false,
    };

    pub const OPEN_FILE: Self = Self {
        can_start: false,
        can_view_source: false,
        can_delete: false,
        can_move: false,
        can_open_file: true,
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemCatalogEntry {
    pub id: ProblemId,
    pub name: String,
    pub source_label: &'static str,
    pub capabilities: ProblemCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemDocument<'a> {
    pub id: ProblemId,
    pub name: String,
    pub source_label: &'static str,
    pub content: Cow<'a, str>,
}

pub trait ProblemSourceProvider {
    fn problem_count(&self) -> usize;
    fn problem_entry(&self, index: usize) -> Option<ProblemCatalogEntry>;
    fn problem_document(&self, index: usize) -> Option<ProblemDocument<'_>>;
}

pub type BuiltinProblemContentProvider = fn(usize) -> &'static str;

pub struct ProblemRepository {
    builtin_names: &'static [&'static str],
    builtin_content: BuiltinProblemContentProvider,
    custom_source_label: &'static str,
    open_file_enabled: bool,
    custom_problems: Vec<CustomProblem>,
}

impl ProblemRepository {
    pub fn new(
        builtin_names: &'static [&'static str],
        builtin_content: BuiltinProblemContentProvider,
        custom_source_label: &'static str,
        open_file_enabled: bool,
    ) -> Self {
        Self {
            builtin_names,
            builtin_content,
            custom_source_label,
            open_file_enabled,
            custom_problems: Vec::new(),
        }
    }

    pub fn set_custom_problems(&mut self, problems: Vec<CustomProblem>) {
        self.custom_problems = problems;
    }

    pub fn custom_problems(&self) -> &[CustomProblem] {
        &self.custom_problems
    }

    pub fn add_custom_problem(&mut self, problem: CustomProblem) -> usize {
        self.custom_problems.push(problem);
        self.builtin_names.len() + self.custom_problems.len() - 1
    }

    pub fn is_open_file_entry(&self, index: usize) -> bool {
        self.open_file_enabled && index == self.builtin_names.len() + self.custom_problems.len()
    }

    pub fn is_custom_problem(&self, index: usize) -> bool {
        self.custom_index(index).is_some()
    }

    pub fn problem_name_at(&self, index: usize) -> Option<&str> {
        if index < self.builtin_names.len() {
            Some(self.builtin_names[index])
        } else if let Some(custom_index) = self.custom_index(index) {
            Some(&self.custom_problems[custom_index].name)
        } else if self.is_open_file_entry(index) {
            Some("[ Open File... ]")
        } else {
            None
        }
    }

    pub fn problem_source_label(&self, index: usize) -> &'static str {
        if index < self.builtin_names.len() {
            "B"
        } else if self.custom_index(index).is_some() {
            self.custom_source_label
        } else if self.is_open_file_entry(index) {
            "+"
        } else {
            "?"
        }
    }

    pub fn problem_content_ref(&self, index: usize) -> Option<&str> {
        if index < self.builtin_names.len() {
            Some((self.builtin_content)(index))
        } else {
            self.custom_index(index)
                .map(|custom_index| self.custom_problems[custom_index].content.as_str())
        }
    }

    pub fn problem_content(&self, index: usize) -> Option<Cow<'_, str>> {
        self.problem_content_ref(index).map(Cow::Borrowed)
    }

    pub fn delete_custom_problem_at(&mut self, index: usize) -> bool {
        let Some(custom_index) = self.custom_index(index) else {
            return false;
        };
        self.custom_problems.remove(custom_index);
        true
    }

    pub fn move_custom_problem_up_at(&mut self, index: usize) -> bool {
        let Some(custom_index) = self.custom_index(index) else {
            return false;
        };
        if custom_index == 0 {
            return false;
        }
        self.custom_problems.swap(custom_index, custom_index - 1);
        true
    }

    pub fn move_custom_problem_down_at(&mut self, index: usize) -> bool {
        let Some(custom_index) = self.custom_index(index) else {
            return false;
        };
        if custom_index + 1 >= self.custom_problems.len() {
            return false;
        }
        self.custom_problems.swap(custom_index, custom_index + 1);
        true
    }

    fn custom_index(&self, index: usize) -> Option<usize> {
        let custom_index = index.checked_sub(self.builtin_names.len())?;
        (custom_index < self.custom_problems.len()).then_some(custom_index)
    }
}

impl ProblemSourceProvider for ProblemRepository {
    fn problem_count(&self) -> usize {
        self.builtin_names.len() + self.custom_problems.len() + usize::from(self.open_file_enabled)
    }

    fn problem_entry(&self, index: usize) -> Option<ProblemCatalogEntry> {
        let source_label = self.problem_source_label(index);
        let name = self.problem_name_at(index)?.to_string();
        let (id, capabilities) = if index < self.builtin_names.len() {
            (ProblemId::builtin(index), ProblemCapabilities::BUILTIN)
        } else if let Some(custom_index) = self.custom_index(index) {
            (ProblemId::custom(custom_index), ProblemCapabilities::CUSTOM)
        } else if self.is_open_file_entry(index) {
            (ProblemId::open_file(index), ProblemCapabilities::OPEN_FILE)
        } else {
            return None;
        };

        Some(ProblemCatalogEntry {
            id,
            name,
            source_label,
            capabilities,
        })
    }

    fn problem_document(&self, index: usize) -> Option<ProblemDocument<'_>> {
        let entry = self.problem_entry(index)?;
        let content = self.problem_content(index)?;
        Some(ProblemDocument {
            id: entry.id,
            name: entry.name,
            source_label: entry.source_label,
            content,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundledFont {
    YujiSyukuRegular,
    MaShanZhengRegular,
    LongCangRegular,
    AlegreyaRegular,
    KalamRegular,
    NotoSerifJpRegular,
}

impl BundledFont {
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::YujiSyukuRegular => "YujiSyuku-Regular.ttf",
            Self::MaShanZhengRegular => "MaShanZheng-Regular.ttf",
            Self::LongCangRegular => "LongCang-Regular.ttf",
            Self::AlegreyaRegular => "Alegreya-VariableFont_wght.ttf",
            Self::KalamRegular => "Kalam-Regular.ttf",
            Self::NotoSerifJpRegular => "NotoSerifJP-Regular.ttf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FontAssetId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontSource {
    Bundled,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontEntry {
    pub id: FontAssetId,
    pub name: String,
    pub source: FontSource,
}

include!(concat!(env!("OUT_DIR"), "/bundled_font_files.rs"));

pub trait AssetProvider {
    fn load_bundled_font(&self, font: BundledFont) -> Result<Vec<u8>, ProviderError>;
    fn list_fonts(&self) -> Vec<FontEntry>;
    fn load_font(&self, id: FontAssetId) -> Result<Vec<u8>, ProviderError>;
}

pub trait PersistentStore {
    fn load_custom_problems(&self) -> Result<Vec<CustomProblem>, ProviderError>;
    fn save_custom_problems(&self, problems: &[CustomProblem]) -> Result<(), ProviderError>;
}

pub trait Clock {
    fn now_ms(&self) -> f64;
}

pub trait Logger {
    fn log(&self, message: &str);
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoopLogger;

impl Logger for NoopLogger {
    fn log(&self, _message: &str) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedProblem {
    pub name: String,
    pub content: String,
    pub timestamp_ms: u64,
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Debug, Clone)]
struct DesktopFontAsset {
    entry: FontEntry,
    path: std::path::PathBuf,
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Debug, Clone)]
pub struct DesktopAssetProvider {
    fonts: Vec<DesktopFontAsset>,
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
impl DesktopAssetProvider {
    pub fn discover() -> Self {
        let mut provider = Self { fonts: Vec::new() };
        provider.discover_fonts();
        provider
    }

    fn discover_fonts(&mut self) {
        let mut search_dirs: Vec<(std::path::PathBuf, FontSource)> = Vec::new();

        for dir in bundled_font_dirs() {
            search_dirs.push((dir, FontSource::Bundled));
        }

        #[cfg(target_os = "windows")]
        {
            search_dirs.push((
                std::path::PathBuf::from(r"C:\Windows\Fonts"),
                FontSource::System,
            ));
        }
        #[cfg(target_os = "macos")]
        {
            search_dirs.push((
                std::path::PathBuf::from("/System/Library/Fonts"),
                FontSource::System,
            ));
            search_dirs.push((
                std::path::PathBuf::from("/Library/Fonts"),
                FontSource::System,
            ));
            if let Ok(home) = std::env::var("HOME") {
                search_dirs.push((
                    std::path::PathBuf::from(home).join("Library/Fonts"),
                    FontSource::System,
                ));
            }
        }
        #[cfg(target_os = "linux")]
        {
            search_dirs.push((
                std::path::PathBuf::from("/usr/share/fonts"),
                FontSource::System,
            ));
            search_dirs.push((
                std::path::PathBuf::from("/usr/local/share/fonts"),
                FontSource::System,
            ));
            if let Ok(home) = std::env::var("HOME") {
                search_dirs.push((
                    std::path::PathBuf::from(home).join(".local/share/fonts"),
                    FontSource::System,
                ));
            }
        }

        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (dir, source) in search_dirs {
            let Ok(read_dir) = std::fs::read_dir(&dir) else {
                continue;
            };

            for entry in read_dir.flatten() {
                let path = entry.path();
                let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
                    continue;
                };
                let ext_lower = ext.to_ascii_lowercase();
                if ext_lower != "ttf" && ext_lower != "otf" {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                    continue;
                };
                let name = stem.to_string();
                if !seen_names.insert(name.clone()) {
                    continue;
                }
                let id = FontAssetId(self.fonts.len());
                self.fonts.push(DesktopFontAsset {
                    entry: FontEntry { id, name, source },
                    path,
                });
            }
        }
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
impl AssetProvider for DesktopAssetProvider {
    fn load_bundled_font(&self, font: BundledFont) -> Result<Vec<u8>, ProviderError> {
        let file_name = font.file_name();
        for dir in bundled_font_dirs() {
            let path = dir.join(file_name);
            if let Ok(bytes) = std::fs::read(&path) {
                return Ok(bytes);
            }
        }

        Err(ProviderError::new(
            ProviderErrorKind::NotFound,
            format!("bundled font not found: {file_name}"),
        ))
    }

    fn list_fonts(&self) -> Vec<FontEntry> {
        self.fonts.iter().map(|font| font.entry.clone()).collect()
    }

    fn load_font(&self, id: FontAssetId) -> Result<Vec<u8>, ProviderError> {
        let Some(asset) = self.fonts.get(id.0) else {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidId,
                format!("unknown font asset id: {}", id.0),
            ));
        };
        std::fs::read(&asset.path).map_err(|error| {
            ProviderError::new(
                ProviderErrorKind::Io,
                format!("failed to read font '{}': {error}", asset.entry.name),
            )
        })
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn bundled_font_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.join("fonts"));
        }
    }
    dirs.push(std::path::PathBuf::from("fonts"));
    dirs
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Debug, Clone, Copy, Default)]
pub struct DesktopProblemSourceProvider;

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
impl DesktopProblemSourceProvider {
    pub fn load_file(
        path: &std::path::Path,
        timestamp_ms: u64,
    ) -> Result<ImportedProblem, ProviderError> {
        let content = std::fs::read_to_string(path).map_err(|error| {
            ProviderError::new(
                ProviderErrorKind::Io,
                format!("failed to read problem file '{}': {error}", path.display()),
            )
        })?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown.ntq")
            .to_string();

        Ok(ImportedProblem {
            name,
            content,
            timestamp_ms,
        })
    }
}
