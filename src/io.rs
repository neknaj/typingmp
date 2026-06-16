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

pub const FONT_DOWNLOAD_BASE_URL: &str = "https://neknaj.github.io/typingmp/fonts";

pub fn embedded_alegreya_font_bytes() -> &'static [u8] {
    include_bytes!("../fonts/Alegreya-VariableFont_wght.ttf")
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
    kind: DesktopFontAssetKind,
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
#[derive(Debug, Clone)]
enum DesktopFontAssetKind {
    Bundled(FontAssetId),
    LocalPath(std::path::PathBuf),
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

        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for entry in bundled_font_entries() {
            seen_names.insert(entry.name.clone());
            self.fonts.push(DesktopFontAsset {
                kind: DesktopFontAssetKind::Bundled(entry.id),
                entry,
            });
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
                    kind: DesktopFontAssetKind::LocalPath(path),
                });
            }
        }
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
impl AssetProvider for DesktopAssetProvider {
    fn load_bundled_font(&self, font: BundledFont) -> Result<Vec<u8>, ProviderError> {
        load_bundled_font_file(font.file_name())
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
        match &asset.kind {
            DesktopFontAssetKind::Bundled(id) => {
                let Some(file_name) = bundled_font_file_name(*id) else {
                    return Err(ProviderError::new(
                        ProviderErrorKind::InvalidId,
                        format!("unknown bundled font asset id: {}", id.0),
                    ));
                };
                load_bundled_font_file(file_name)
            }
            DesktopFontAssetKind::LocalPath(path) => std::fs::read(path).map_err(|error| {
                ProviderError::new(
                    ProviderErrorKind::Io,
                    format!("failed to read font '{}': {error}", asset.entry.name),
                )
            }),
        }
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn load_bundled_font_file(file_name: &str) -> Result<Vec<u8>, ProviderError> {
    if file_name == BundledFont::AlegreyaRegular.file_name() {
        return Ok(embedded_alegreya_font_bytes().to_vec());
    }

    let mut cache_error = None;
    match read_cached_font(file_name) {
        Ok(Some(bytes)) => match validate_font_bytes(file_name, &bytes) {
            Ok(()) => return Ok(bytes),
            Err(error) => {
                cache_error = Some(error);
                remove_cached_font(file_name);
            }
        },
        Ok(None) => {}
        Err(error) => cache_error = Some(error),
    }

    match fetch_and_cache_font(file_name) {
        Ok(bytes) => Ok(bytes),
        Err(fetch_error) => match read_local_bundled_font(file_name) {
            Ok(bytes) => Ok(bytes),
            Err(local_error) => Err(ProviderError::new(
                ProviderErrorKind::NotFound,
                format_bundled_font_load_error(file_name, cache_error, fetch_error, local_error),
            )),
        },
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn read_local_bundled_font(file_name: &str) -> Result<Vec<u8>, ProviderError> {
    let mut decode_error = None;
    for dir in bundled_font_dirs() {
        let path = dir.join(file_name);
        if let Ok(bytes) = std::fs::read(&path) {
            match validate_font_bytes(file_name, &bytes) {
                Ok(()) => return Ok(bytes),
                Err(error) => decode_error = Some(error),
            }
        }
    }

    if let Some(error) = decode_error {
        return Err(error);
    }

    Err(ProviderError::new(
        ProviderErrorKind::NotFound,
        format!("local bundled font not found: {file_name}"),
    ))
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn read_cached_font(file_name: &str) -> Result<Option<Vec<u8>>, ProviderError> {
    let Some(path) = desktop_cached_font_path(file_name) else {
        return Ok(None);
    };
    match std::fs::read(&path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(ProviderError::new(
            ProviderErrorKind::Io,
            format!("failed to read cached font '{}': {error}", path.display()),
        )),
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn validate_font_bytes(file_name: &str, bytes: &[u8]) -> Result<(), ProviderError> {
    ab_glyph::FontVec::try_from_vec(bytes.to_vec())
        .map(|_| ())
        .map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Decode,
                format!("font data could not be parsed: {file_name}"),
            )
        })
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn remove_cached_font(file_name: &str) {
    if let Some(path) = desktop_cached_font_path(file_name) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn format_bundled_font_load_error(
    file_name: &str,
    cache_error: Option<ProviderError>,
    fetch_error: ProviderError,
    local_error: ProviderError,
) -> String {
    match cache_error {
        Some(cache_error) => format!(
            "failed to load bundled font {file_name}: cache: {cache_error}; fetch: {fetch_error}; local fallback: {local_error}"
        ),
        None => format!(
            "failed to load bundled font {file_name}: fetch: {fetch_error}; local fallback: {local_error}"
        ),
    }
}

#[cfg(all(
    not(any(target_arch = "wasm32", feature = "uefi")),
    feature = "desktop-font-fetch"
))]
fn fetch_and_cache_font(file_name: &str) -> Result<Vec<u8>, ProviderError> {
    let url = format!("{FONT_DOWNLOAD_BASE_URL}/{file_name}");
    let response = ureq::get(&url).call().map_err(|error| {
        ProviderError::new(
            ProviderErrorKind::Io,
            format!("failed to fetch font from {url}: {error}"),
        )
    })?;

    let mut bytes = Vec::new();
    let mut reader = response.into_reader();
    std::io::Read::read_to_end(&mut reader, &mut bytes).map_err(|error| {
        ProviderError::new(
            ProviderErrorKind::Io,
            format!("failed to read font response from {url}: {error}"),
        )
    })?;
    if bytes.is_empty() {
        return Err(ProviderError::new(
            ProviderErrorKind::Decode,
            format!("fetched font was empty: {url}"),
        ));
    }
    validate_font_bytes(file_name, &bytes)?;

    if let Some(path) = desktop_cached_font_path(file_name) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &bytes);
    }

    Ok(bytes)
}

#[cfg(all(
    not(any(target_arch = "wasm32", feature = "uefi")),
    not(feature = "desktop-font-fetch")
))]
fn fetch_and_cache_font(file_name: &str) -> Result<Vec<u8>, ProviderError> {
    Err(ProviderError::new(
        ProviderErrorKind::Unsupported,
        format!("font fetching is disabled: {file_name}"),
    ))
}

#[cfg(all(
    not(any(target_arch = "wasm32", feature = "uefi")),
    feature = "desktop-font-fetch"
))]
pub fn desktop_font_cache_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("io.github", "neknaj", "typingmp")
        .map(|dirs| dirs.data_local_dir().join("fonts"))
}

#[cfg(all(
    not(any(target_arch = "wasm32", feature = "uefi")),
    not(feature = "desktop-font-fetch")
))]
pub fn desktop_font_cache_dir() -> Option<std::path::PathBuf> {
    None
}

#[cfg(not(any(target_arch = "wasm32", feature = "uefi")))]
fn desktop_cached_font_path(file_name: &str) -> Option<std::path::PathBuf> {
    desktop_font_cache_dir().map(|dir| dir.join(file_name))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_font_list_always_includes_bundled_font_entries() {
        let provider = DesktopAssetProvider::discover();
        let entries = provider.list_fonts();

        for bundled in bundled_font_entries() {
            assert!(
                entries.iter().any(|entry| entry.id == bundled.id
                    && entry.name == bundled.name
                    && entry.source == FontSource::Bundled),
                "missing bundled font entry: {}",
                bundled.name
            );
        }
    }

    #[test]
    fn desktop_loads_alegreya_from_embedded_bytes() {
        let provider = DesktopAssetProvider::discover();

        let bytes = provider
            .load_bundled_font(BundledFont::AlegreyaRegular)
            .expect("embedded Alegreya should load");

        assert_eq!(bytes.as_slice(), embedded_alegreya_font_bytes());
    }

    #[test]
    fn desktop_font_validation_rejects_corrupt_bytes() {
        assert!(validate_font_bytes("broken.ttf", b"not a font").is_err());
    }

    #[cfg(feature = "desktop-font-fetch")]
    #[test]
    fn desktop_font_cache_dir_points_under_project_fonts() {
        let path = desktop_font_cache_dir().expect("ProjectDirs should resolve on desktop");

        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some("fonts")
        );
    }
}
