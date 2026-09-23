use tauri::{AppHandle, Manager, Runtime};

/// Brings the main window in front of the user, whatever state it was left in.
///
/// Used by the tray, and when FocusBridge is launched a second time: from Start
/// search, a pinned shortcut, or a double-clicked icon while it is already
/// running. The second launch used to die quietly -- the database lock turned it
/// away during setup -- so the user saw nothing at all and assumed the app was
/// broken, when it was sitting hidden in the tray the whole time.
pub fn reveal_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    // A minimized window is still "shown", so `show` alone does nothing for it.
    let _ = window.unminimize();
    let _ = window.show();
    // Windows only lets the foreground process take focus. The second launch was
    // the foreground process, but it is this one that must come forward, so
    // briefly raising it above everything is what makes it actually appear
    // rather than just flash in the taskbar.
    let _ = window.set_always_on_top(true);
    let _ = window.set_always_on_top(false);
    let _ = window.set_focus();
}

/// Takes the browser out of the webview.
///
/// WebView2 ships a browser's context menu and keyboard shortcuts. Right-click
/// offered "Save as", which wrote the whole inbox -- every message on screen --
/// to an HTML file anywhere on disk, and "Print", which does the same on paper.
/// Ctrl+S and Ctrl+P did it without the menu. An app whose job is to keep
/// messages private cannot have a one-click export of all of them, so the menu
/// and the browser shortcuts are switched off at the source. Editing keys (copy,
/// paste, undo, select all) are not browser accelerators and keep working.
pub fn lock_down_webview<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    #[cfg(windows)]
    let reached = window.with_webview(|webview| {
        use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings3;
        use windows_core::Interface;

        // A refused setting must be visible: the whole point of this function
        // is that the inbox cannot be exported, and a silent failure would read
        // in the log as though it had worked.
        fn report(what: &str, result: windows_core::Result<()>) {
            if let Err(error) = result {
                tracing::error!(%error, what, "webview lockdown setting was refused");
            }
        }

        // SAFETY: `with_webview` runs this on the thread that owns the webview,
        // while the controller it hands over is alive; every COM pointer below is
        // obtained from it here and dropped before the closure returns.
        unsafe {
            let core = match webview.controller().CoreWebView2() {
                Ok(core) => core,
                Err(error) => {
                    tracing::error!(%error, "could not reach the webview to lock it down");
                    return;
                }
            };
            let settings = match core.Settings() {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::error!(%error, "could not reach the webview settings to lock them down");
                    return;
                }
            };
            report("context menu", settings.SetAreDefaultContextMenusEnabled(false));
            report("status bar", settings.SetIsStatusBarEnabled(false));
            // Developer tools read the whole page too; they stay for a debug build.
            #[cfg(not(debug_assertions))]
            report("developer tools", settings.SetAreDevToolsEnabled(false));
            match settings.cast::<ICoreWebView2Settings3>() {
                Ok(settings) => report(
                    "browser shortcuts",
                    settings.SetAreBrowserAcceleratorKeysEnabled(false),
                ),
                Err(error) => {
                    tracing::error!(%error, "browser shortcuts could not be switched off")
                }
            }
        }
        tracing::info!("webview locked down: no context menu, no browser shortcuts");
    });
    #[cfg(windows)]
    if let Err(error) = reached {
        tracing::error!(%error, "could not reach the webview to lock it down");
    }
    #[cfg(not(windows))]
    let _ = window;
}
