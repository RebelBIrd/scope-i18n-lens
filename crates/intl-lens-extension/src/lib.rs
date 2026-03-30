use zed_extension_api::{self as zed, LanguageServerId, Result, Worktree};

const PRIMARY_BINARY_NAME: &str = "scope-i18n-lens";
const LEGACY_BINARY_NAME: &str = "intl-lens";
const PRIMARY_REPOSITORY: &str = "BigHuang/scope-i18n-lens";
const LEGACY_REPOSITORY: &str = "nguyenphutrong/intl-lens";

struct ScopeI18nLensExtension {
    cached_binary_path: Option<String>,
}

impl zed::Extension for ScopeI18nLensExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        let binary_path = self.get_server_binary_path(language_server_id, worktree)?;

        Ok(zed::Command {
            command: binary_path,
            args: vec![],
            env: vec![],
        })
    }
}

impl ScopeI18nLensExtension {
    fn get_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if std::fs::metadata(path).is_ok() {
                return Ok(path.clone());
            }
        }

        if let Some(path) = worktree.which(PRIMARY_BINARY_NAME) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        if let Some(path) = worktree.which(LEGACY_BINARY_NAME) {
            self.cached_binary_path = Some(path.clone());
            return Ok(path);
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(PRIMARY_REPOSITORY, Self::release_options())
            .or_else(|_| zed::latest_github_release(LEGACY_REPOSITORY, Self::release_options()))?;

        let (platform, arch) = zed::current_platform();
        let platform_suffix = match platform {
            zed::Os::Mac => "apple-darwin",
            zed::Os::Linux => "unknown-linux-gnu",
            zed::Os::Windows => "pc-windows-msvc",
        };
        let arch_suffix = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X8664 => "x86_64",
            zed::Architecture::X86 => "x86",
        };
        let archive_ext = match platform {
            zed::Os::Windows => "zip",
            _ => "tar.gz",
        };

        let candidate_asset_names = [
            format!(
                "{}-{}-{}.{}",
                PRIMARY_BINARY_NAME, arch_suffix, platform_suffix, archive_ext
            ),
            format!(
                "{}-{}-{}.{}",
                LEGACY_BINARY_NAME, arch_suffix, platform_suffix, archive_ext
            ),
        ];

        let asset = release
            .assets
            .iter()
            .find(|asset| candidate_asset_names.iter().any(|name| name == &asset.name))
            .ok_or_else(|| {
                format!(
                    "no release asset found for platform (expected one of: {})",
                    candidate_asset_names.join(", ")
                )
            })?;

        let binary_name = if asset.name.starts_with(PRIMARY_BINARY_NAME) {
            PRIMARY_BINARY_NAME
        } else {
            LEGACY_BINARY_NAME
        };

        let version_dir = format!("{}-{}", binary_name, release.version);
        let binary_path = format!(
            "{version_dir}/{}{}",
            binary_name,
            match platform {
                zed::Os::Windows => ".exe",
                _ => "",
            }
        );

        if std::fs::metadata(&binary_path).is_err() {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            let file_type = match platform {
                zed::Os::Windows => zed::DownloadedFileType::Zip,
                _ => zed::DownloadedFileType::GzipTar,
            };

            zed::download_file(&asset.download_url, &version_dir, file_type)?;

            zed::make_file_executable(&binary_path)?;
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    fn release_options() -> zed::GithubReleaseOptions {
        zed::GithubReleaseOptions {
            require_assets: true,
            pre_release: false,
        }
    }
}

zed::register_extension!(ScopeI18nLensExtension);
