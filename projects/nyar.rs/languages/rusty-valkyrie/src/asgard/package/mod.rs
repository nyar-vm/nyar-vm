//! VOA dist 打包：manifest、小程序、Android、iOS（二进制交付）。

pub mod android;
pub mod artifact;
pub mod desktop;
pub mod ios;
pub mod manifest;
pub mod miniprogram;
pub mod terminal;

pub use android::{AndroidPackageOutput, package_android_project};
pub use artifact::PackageArtifact;
pub use desktop::{DesktopPackageOutput, package_desktop_project};
pub use ios::{IosPackageOutput, package_ios_project};
pub use manifest::{ManifestOutput, ManifestUrlMode, generate_manifest_with_urls};
pub use miniprogram::{MpAppJsonOutput, MpPackageOutput, generate_app_json, package_miniprogram_pages};
pub use terminal::{TerminalPackageOutput, package_terminal_project};
