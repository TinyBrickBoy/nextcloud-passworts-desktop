// Verhindert unter Windows ein zusätzliches Konsolenfenster im Release Build.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK bricht mit "Could not create default EGL display: EGL_BAD_PARAMETER" ab,
    // wenn der DMA-BUF Renderer kein EGL bekommt (z.B. AppImage in VMs ohne GPU).
    // Den automatischen Workaround auf AppImage-Starts begrenzen (auch entpackte AppImages).
    // Ein vom Nutzer gesetzter Wert bleibt erhalten. Läuft vor jedem weiteren Thread.
    #[cfg(target_os = "linux")]
    if (std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some())
        && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
    {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    ncpw_lib::run()
}
