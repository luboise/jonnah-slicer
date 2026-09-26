#[expect(
    clippy::allow_attributes,
    reason = "the expect won't trigger when we aren't on windows'"
)]
#[allow(clippy::unnecessary_wraps, reason = "we need this result on windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    set_icon()?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn set_icon() -> Result<(), Box<dyn std::error::Error>> {
    const ICON_PATH: &str = "assets/jonnah.ico";

    let mut res = winresource::WindowsResource::new();
    res.set_icon(ICON_PATH)
        // manually set version 1.0.0.0
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, 0x0001000000000000);
    res.compile()?;

    Ok(())
}
